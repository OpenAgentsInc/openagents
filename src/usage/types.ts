import * as S from "effect/Schema";

export const UsageRecordSchema = S.Struct({
  sessionId: S.String,
  projectId: S.String,
  timestamp: S.String, // ISO8601
  inputTokens: S.Number,
  outputTokens: S.Number,
  cacheReadTokens: S.Number,
  cacheCreationTokens: S.Number,
  totalCostUsd: S.Number,
  agent: S.Literal("claude-code", "minimal", "grok", "openai", "mixed", "unknown"),
  subtasks: S.Number,
  durationMs: S.Number,
});

export type UsageRecord = S.Schema.Type<typeof UsageRecordSchema>;

export interface UsageTotals {
  sessions: number;
  subtasks: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalCostUsd: number;
}

export interface UsageSummary {
  period: "day" | "week" | "month";
  byPeriod: Record<string, UsageTotals>;
  overall: UsageTotals;
}

export const createEmptyTotals = (): UsageTotals => ({
  sessions: 0,
  subtasks: 0,
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
  totalCostUsd: 0,
});
