import { Exit, Schema } from "@effect-native/core/effect"

import { WorkbenchItemSchema } from "./workbench-item-contract.ts"

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
const Text = Schema.String.check(Schema.isMaxLength(20_000))
const Timestamp = Schema.String.check(Schema.isMaxLength(64))
const Count = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))
const CodexHistoryAgentStatusSchema = Schema.Literals(["pending", "running", "waiting", "interrupted", "completed", "errored", "shutdown", "not_found", "unknown"])

/**
 * Provider that produced a history node. Import is additive (#8712 H3): the
 * catalog carries Codex and Claude sessions side by side, tagged so the UI can
 * badge the source. It never changes loss-accounting — each source still
 * projects its own whole-conversation completeness equation.
 */
export const CodexHistorySourceSchema = Schema.Literals(["codex", "claude"])
export type CodexHistorySource = typeof CodexHistorySourceSchema.Type

export const CodexHistoryAgentSchema = Schema.Struct({
  threadRef: Ref,
  parentThreadRef: Schema.NullOr(Ref),
  title: Schema.String.check(Schema.isMaxLength(160)),
  status: CodexHistoryAgentStatusSchema,
  createdAt: Timestamp,
  updatedAt: Timestamp,
  depth: Count,
  descendantCount: Count,
  model: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
  role: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
  nickname: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
  agentPath: Schema.NullOr(Schema.String.check(Schema.isMaxLength(1_024))),
  sourceVersion: Schema.NullOr(Schema.String.check(Schema.isMaxLength(80))),
  reasoning: Schema.NullOr(Schema.String.check(Schema.isMaxLength(160))),
  source: CodexHistorySourceSchema,
  /**
   * A discovered child whose provider edge could not be structurally
   * recovered (the ~3% Claude orphan class per the subagents teardown). Shown
   * and counted as an explicit topology gap, never silently hidden.
   */
  orphan: Schema.Boolean.pipe(Schema.optionalKey),
})
export type CodexHistoryAgent = typeof CodexHistoryAgentSchema.Type

export const CodexHistoryItemKindSchema = Schema.Literals([
  "session", "context", "metadata", "user_message", "assistant_message", "agent_message", "system_message", "reasoning", "plan",
  "collaboration", "tool_call", "tool_result", "approval", "usage", "lifecycle", "error", "gap",
])
export type CodexHistoryItemKind = typeof CodexHistoryItemKindSchema.Type

export const CodexHistoryAgentPreviewSchema = Schema.Struct({
  threadRef: Ref,
  title: Schema.String.check(Schema.isMaxLength(160)),
  status: CodexHistoryAgentStatusSchema,
  updatedAt: Timestamp,
  latest: Schema.NullOr(Schema.Struct({
    label: Schema.String.check(Schema.isMaxLength(160)),
    summary: Schema.String.check(Schema.isMaxLength(360)),
    kind: CodexHistoryItemKindSchema,
    timestamp: Timestamp,
  })),
})
export type CodexHistoryAgentPreview = typeof CodexHistoryAgentPreviewSchema.Type

export const CodexHistoryFieldSchema = Schema.Struct({ label: Schema.String.check(Schema.isMaxLength(80)), value: Text })
export const CodexHistoryItemSchema = Schema.Struct({
  itemRef: Ref,
  threadRef: Ref,
  sequence: Count,
  timestamp: Timestamp,
  kind: CodexHistoryItemKindSchema,
  label: Schema.String.check(Schema.isMaxLength(160)),
  summary: Text,
  status: Schema.NullOr(Schema.String.check(Schema.isMaxLength(80))),
  fields: Schema.Array(CodexHistoryFieldSchema).check(Schema.isMaxLength(40)),
  redacted: Schema.Boolean,
  sourceType: Schema.String.check(Schema.isMaxLength(160)),
  relatedAgent: CodexHistoryAgentPreviewSchema.pipe(Schema.optionalKey),
  /**
   * Typed sidecar (#8859): the structured `WorkbenchItem` a renderer needs to
   * rebuild the same typed card the live turn showed (command cwd/exit/
   * duration/output tail, per-file diffs, tool args/results). Additive —
   * absent on rows the projection has no tool-class source record for, and
   * the bounded label/summary/fields presentation stays authoritative for
   * loss accounting.
   */
  item: Schema.optional(WorkbenchItemSchema),
})
export type CodexHistoryItem = typeof CodexHistoryItemSchema.Type

export const CodexHistoryCompletenessSchema = Schema.Struct({
  source: Count,
  rendered: Count,
  redactions: Count,
  gaps: Count,
  complete: Schema.Boolean,
})
export type CodexHistoryCompleteness = typeof CodexHistoryCompletenessSchema.Type

export const CodexHistoryCatalogSchema = Schema.Struct({
  roots: Schema.Array(CodexHistoryAgentSchema).check(Schema.isMaxLength(10_000)),
  agents: Schema.Array(CodexHistoryAgentSchema).check(Schema.isMaxLength(10_000)),
})
export type CodexHistoryCatalog = typeof CodexHistoryCatalogSchema.Type

export const CodexHistoryPageSchema = Schema.Struct({
  rootThreadRef: Ref,
  selectedThreadRef: Ref,
  agents: Schema.Array(CodexHistoryAgentSchema).check(Schema.isMaxLength(10_000)),
  items: Schema.Array(CodexHistoryItemSchema).check(Schema.isMaxLength(500)),
  offset: Count,
  limit: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 500 })),
  totalItems: Count,
  hasPrevious: Schema.Boolean,
  hasNext: Schema.Boolean,
  completeness: CodexHistoryCompletenessSchema,
})
export type CodexHistoryPage = typeof CodexHistoryPageSchema.Type

/**
 * Free-text session search (#8712 H4). A rebuildable local cache over session
 * TITLES and a bounded per-session content index — NOT authority (per the
 * audit's History/Discovery/Memory split, indexes are caches). Each result
 * points at the session root plus, for a content match, the exact matching
 * item so the UI can open the session windowed on that item.
 */
export const CodexHistorySearchResultSchema = Schema.Struct({
  threadRef: Ref,
  rootThreadRef: Ref,
  source: CodexHistorySourceSchema,
  title: Schema.String.check(Schema.isMaxLength(160)),
  matchKind: Schema.Literals(["title", "content"]),
  matchItemRef: Schema.NullOr(Ref),
  matchSequence: Schema.NullOr(Count),
  snippet: Schema.String.check(Schema.isMaxLength(240)),
  updatedAt: Timestamp,
  score: Schema.Number,
})
export type CodexHistorySearchResult = typeof CodexHistorySearchResultSchema.Type

export const CodexHistorySearchResponseSchema = Schema.Struct({
  query: Schema.String.check(Schema.isMaxLength(200)),
  results: Schema.Array(CodexHistorySearchResultSchema).check(Schema.isMaxLength(100)),
  /** How many sessions the bounded content index actually scanned this pass. */
  indexedSessions: Count,
  /** True when the bounded index budget stopped short of every session. */
  truncated: Schema.Boolean,
})
export type CodexHistorySearchResponse = typeof CodexHistorySearchResponseSchema.Type

const decode = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return Exit.isSuccess(result) ? result.value as A : null
}
export const decodeCodexHistoryCatalog = (value: unknown): CodexHistoryCatalog | null => decode(CodexHistoryCatalogSchema, value)
export const decodeCodexHistoryPage = (value: unknown): CodexHistoryPage | null => decode(CodexHistoryPageSchema, value)
export const decodeCodexHistorySearchResponse = (value: unknown): CodexHistorySearchResponse | null => decode(CodexHistorySearchResponseSchema, value)
