import { Schema as S } from "effect";

export const ProbeLlmProviderMetadata = S.Record(S.String, S.Unknown);
export type ProbeLlmProviderMetadata = typeof ProbeLlmProviderMetadata.Type;

export const ProbeLlmUsage = S.Struct({
  inputTokens: S.optional(S.Number),
  outputTokens: S.optional(S.Number),
  nonCachedInputTokens: S.optional(S.Number),
  cacheReadInputTokens: S.optional(S.Number),
  cacheWriteInputTokens: S.optional(S.Number),
  reasoningTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  providerMetadata: S.optional(ProbeLlmProviderMetadata),
});
export type ProbeLlmUsage = typeof ProbeLlmUsage.Type;

export function makeProbeLlmUsage(input: ProbeLlmUsage): ProbeLlmUsage {
  const outputTokens = input.outputTokens;
  const reasoningTokens =
    outputTokens === undefined || input.reasoningTokens === undefined
      ? input.reasoningTokens
      : Math.min(Math.max(0, input.reasoningTokens), outputTokens);
  const totalTokens = input.totalTokens ?? sumDefined(input.inputTokens, outputTokens);

  return {
    ...input,
    reasoningTokens,
    totalTokens,
  };
}

export function probeLlmVisibleOutputTokens(usage: ProbeLlmUsage): number {
  return Math.max(0, (usage.outputTokens ?? 0) - (usage.reasoningTokens ?? 0));
}

function sumDefined(left: number | undefined, right: number | undefined): number | undefined {
  return left === undefined || right === undefined ? undefined : left + right;
}
