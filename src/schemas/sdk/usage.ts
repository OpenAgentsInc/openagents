/**
 * SDK-compatible usage and cost schemas.
 *
 * Mirrors the Claude Agent SDK usage structures while providing
 * a non-nullable variant that defaults missing fields to 0.
 */

import * as S from "effect/Schema";

const tokenFields = {
  input_tokens: S.optional(S.Number),
  output_tokens: S.optional(S.Number),
  cache_read_input_tokens: S.optional(S.Number),
  cache_creation_input_tokens: S.optional(S.Number),
} as const;

const costFields = {
  input_cost_usd: S.optional(S.Number),
  output_cost_usd: S.optional(S.Number),
  cache_read_cost_usd: S.optional(S.Number),
  cache_creation_cost_usd: S.optional(S.Number),
  total_cost_usd: S.optional(S.Number),
} as const;

/**
 * Token usage metrics from the provider.
 */
export const TokenUsage = S.Struct(tokenFields);

/**
 * Cost breakdown in USD for the request/response.
 */
export const CostBreakdown = S.Struct(costFields);

const NullableUsage = S.Struct({
  ...tokenFields,
  ...costFields,
});

const NonNullableUsageStruct = S.Struct({
  input_tokens: S.Number,
  output_tokens: S.Number,
  cache_read_input_tokens: S.Number,
  cache_creation_input_tokens: S.Number,
  input_cost_usd: S.Number,
  output_cost_usd: S.Number,
  cache_read_cost_usd: S.Number,
  cache_creation_cost_usd: S.Number,
  total_cost_usd: S.Number,
});

/**
 * Usage metrics with all fields required and defaulted to zero.
 *
 * When total_cost_usd is not provided, it is derived from the
 * individual cost components.
 */
export const NonNullableUsage = S.transform(
  NullableUsage,
  NonNullableUsageStruct,
  {
    decode: (value) => ({
      input_tokens: value.input_tokens ?? 0,
      output_tokens: value.output_tokens ?? 0,
      cache_read_input_tokens: value.cache_read_input_tokens ?? 0,
      cache_creation_input_tokens: value.cache_creation_input_tokens ?? 0,
      input_cost_usd: value.input_cost_usd ?? 0,
      output_cost_usd: value.output_cost_usd ?? 0,
      cache_read_cost_usd: value.cache_read_cost_usd ?? 0,
      cache_creation_cost_usd: value.cache_creation_cost_usd ?? 0,
      total_cost_usd:
        value.total_cost_usd ??
        (value.input_cost_usd ?? 0) +
          (value.output_cost_usd ?? 0) +
          (value.cache_read_cost_usd ?? 0) +
          (value.cache_creation_cost_usd ?? 0),
    }),
    encode: (value) => value,
  },
);

export type TokenUsage = S.Schema.Type<typeof TokenUsage>;
export type CostBreakdown = S.Schema.Type<typeof CostBreakdown>;
export type NonNullableUsage = S.Schema.Type<typeof NonNullableUsage>;
