import { Effect, Option, Schema } from "effect";

import { BlobRefSchema, type BlobRef } from "../blob.js";
import { canonicalJson } from "../internal/canonicalJson.js";

import type { DseParams } from "../params.js";

import type { BudgetHandle } from "./budget.js";
import { BudgetExceededError } from "./budget.js";
import { BlobStoreError, BlobStoreService } from "./blobStore.js";
import { LmClientError, LmClientService, type LmMessage } from "./lm.js";
import { ToolCallError, ToolExecutorService } from "./toolExecutor.js";
import { VarSpaceError, VarSpaceService } from "./varSpace.js";

export class RlmKernelError extends Schema.TaggedError<RlmKernelError>()(
  "RlmKernelError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

/**
 * Phase H: Poisoning/Confusion hardening primitives.
 *
 * We treat all variable-space text as untrusted by default and attach provenance
 * (SpanRef-like metadata) to every excerpt that enters token space via an Observation.
 */
export type RlmTrustLabelV1 = "trusted" | "semi_trusted" | "untrusted";

export type RlmContentOriginV1 = "input" | "tool" | "lm" | "derived" | "unknown";

export const RlmTrustLabelV1Schema: Schema.Schema<RlmTrustLabelV1> = Schema.Literal(
  "trusted",
  "semi_trusted",
  "untrusted"
);

export const RlmContentOriginV1Schema: Schema.Schema<RlmContentOriginV1> =
  Schema.Literal("input", "tool", "lm", "derived", "unknown");

export type RlmTargetV1 =
  | { readonly _tag: "Var"; readonly name: string }
  | { readonly _tag: "Blob"; readonly blobId: string };

export const RlmTargetV1Schema: Schema.Schema<RlmTargetV1> = Schema.Union(
  Schema.Struct({ _tag: Schema.Literal("Var"), name: Schema.String }),
  Schema.Struct({ _tag: Schema.Literal("Blob"), blobId: Schema.String })
);

export type RlmSourceV1 =
  | { readonly _tag: "Blob"; readonly blobId: string }
  | { readonly _tag: "VarJson"; readonly name: string }
  | { readonly _tag: "VarBlob"; readonly name: string; readonly blobId: string };

export const RlmSourceV1Schema: Schema.Schema<RlmSourceV1> = Schema.Union(
  Schema.Struct({ _tag: Schema.Literal("Blob"), blobId: Schema.String }),
  Schema.Struct({ _tag: Schema.Literal("VarJson"), name: Schema.String }),
  Schema.Struct({
    _tag: Schema.Literal("VarBlob"),
    name: Schema.String,
    blobId: Schema.String
  })
);

export type RlmSpanRefV1 = {
  readonly _tag: "SpanRef";
  readonly source: RlmSourceV1;
  readonly startChar: number;
  readonly endChar: number;
  readonly totalChars: number;
  readonly startLine?: number | undefined;
  readonly endLine?: number | undefined;
};

export const RlmSpanRefV1Schema: Schema.Schema<RlmSpanRefV1> = Schema.Struct({
  _tag: Schema.Literal("SpanRef"),
  source: RlmSourceV1Schema,
  startChar: Schema.Number,
  endChar: Schema.Number,
  totalChars: Schema.Number,
  startLine: Schema.optional(Schema.Number),
  endLine: Schema.optional(Schema.Number)
});

export type RlmModelConfigV1 = {
  readonly modelId?: string | undefined;
  readonly temperature?: number | undefined;
  readonly topP?: number | undefined;
  readonly maxTokens?: number | undefined;
};

export const RlmModelConfigV1Schema: Schema.Schema<RlmModelConfigV1> =
  Schema.Struct({
    modelId: Schema.optional(Schema.String),
    temperature: Schema.optional(Schema.Number),
    topP: Schema.optional(Schema.Number),
    maxTokens: Schema.optional(Schema.Number)
  });

export type RlmWriteValueV1 =
  | { readonly _tag: "Json"; readonly value: unknown }
  | { readonly _tag: "Blob"; readonly blob: BlobRef };

export const RlmWriteValueV1Schema: Schema.Schema<RlmWriteValueV1> = Schema.Union(
  Schema.Struct({ _tag: Schema.Literal("Json"), value: Schema.Unknown }),
  Schema.Struct({ _tag: Schema.Literal("Blob"), blob: BlobRefSchema })
);

export type RlmActionV1 =
  | {
      readonly _tag: "Preview";
      readonly target: RlmTargetV1;
      readonly offset?: number | undefined;
      readonly length?: number | undefined;
    }
  | {
      readonly _tag: "Search";
      readonly target: RlmTargetV1;
      readonly query: string;
      readonly maxMatches?: number | undefined;
      readonly contextChars?: number | undefined;
    }
  | {
      readonly _tag: "Load";
      readonly blobId: string;
      readonly intoVar: string;
    }
  | {
      readonly _tag: "Chunk";
      readonly target: RlmTargetV1;
      readonly chunkChars: number;
      readonly overlapChars?: number | undefined;
      readonly maxChunks?: number | undefined;
      readonly intoVar: string;
    }
  | {
      readonly _tag: "WriteVar";
      readonly name: string;
      readonly value: RlmWriteValueV1;
    }
  | {
      readonly _tag: "SubLm";
      readonly messages: ReadonlyArray<LmMessage>;
      readonly model?: RlmModelConfigV1 | undefined;
      readonly intoVar: string;
    }
  | {
      readonly _tag: "ExtractOverChunks";
      readonly chunksVar: string;
      readonly instruction: string;
      readonly model?: RlmModelConfigV1 | undefined;
      readonly maxChunks?: number | undefined;
      readonly intoVar: string;
    }
  | {
      readonly _tag: "ToolCall";
      readonly toolName: string;
      readonly input: unknown;
      readonly timeoutMs?: number | undefined;
      readonly intoVar: string;
    }
  | { readonly _tag: "Final"; readonly output: unknown };

const LmMessageSchema: Schema.Schema<LmMessage> = Schema.Struct({
  role: Schema.Literal("system", "user", "assistant"),
  content: Schema.String
});

export const RlmActionV1Schema: Schema.Schema<RlmActionV1> = Schema.Union(
  Schema.Struct({
    _tag: Schema.Literal("Preview"),
    target: RlmTargetV1Schema,
    offset: Schema.optional(Schema.Number),
    length: Schema.optional(Schema.Number)
  }),
  Schema.Struct({
    _tag: Schema.Literal("Search"),
    target: RlmTargetV1Schema,
    query: Schema.String,
    maxMatches: Schema.optional(Schema.Number),
    contextChars: Schema.optional(Schema.Number)
  }),
  Schema.Struct({
    _tag: Schema.Literal("Load"),
    blobId: Schema.String,
    intoVar: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("Chunk"),
    target: RlmTargetV1Schema,
    chunkChars: Schema.Number,
    overlapChars: Schema.optional(Schema.Number),
    maxChunks: Schema.optional(Schema.Number),
    intoVar: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("WriteVar"),
    name: Schema.String,
    value: RlmWriteValueV1Schema
  }),
  Schema.Struct({
    _tag: Schema.Literal("SubLm"),
    messages: Schema.Array(LmMessageSchema),
    model: Schema.optional(RlmModelConfigV1Schema),
    intoVar: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("ExtractOverChunks"),
    chunksVar: Schema.String,
    instruction: Schema.String,
    model: Schema.optional(RlmModelConfigV1Schema),
    maxChunks: Schema.optional(Schema.Number),
    intoVar: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("ToolCall"),
    toolName: Schema.String,
    input: Schema.Unknown,
    timeoutMs: Schema.optional(Schema.Number),
    intoVar: Schema.String
  }),
  Schema.Struct({ _tag: Schema.Literal("Final"), output: Schema.Unknown })
);

export type RlmObservationV1 =
  | {
      readonly _tag: "PreviewResult";
      readonly trust: RlmTrustLabelV1;
      readonly origin: RlmContentOriginV1;
      readonly target: RlmTargetV1;
      readonly span: RlmSpanRefV1;
      readonly offset: number;
      readonly length: number;
      readonly totalChars: number;
      readonly truncated: boolean;
      readonly text: string;
    }
  | {
      readonly _tag: "SearchResult";
      readonly trust: RlmTrustLabelV1;
      readonly origin: RlmContentOriginV1;
      readonly target: RlmTargetV1;
      readonly totalChars: number;
      readonly query: string;
      readonly totalMatches: number;
      readonly truncated: boolean;
      readonly matches: ReadonlyArray<{
        readonly index: number;
        readonly snippet: string;
        readonly span: RlmSpanRefV1;
      }>;
    }
  | {
      readonly _tag: "LoadResult";
      readonly intoVar: string;
      readonly blob: BlobRef;
    }
  | {
      readonly _tag: "ChunkResult";
      readonly intoVar: string;
      readonly chunkCount: number;
      readonly chunkChars: number;
      readonly overlapChars: number;
    }
  | { readonly _tag: "WriteVarResult"; readonly name: string; readonly kind: "json" | "blob" }
  | {
      readonly _tag: "SubLmResult";
      readonly trust: RlmTrustLabelV1;
      readonly origin: RlmContentOriginV1;
      readonly intoVar: string;
      readonly blob: BlobRef;
    }
  | {
      readonly _tag: "ExtractOverChunksResult";
      readonly intoVar: string;
      readonly chunkCount: number;
      readonly outputsVar: string;
    }
  | {
      readonly _tag: "ToolCallResult";
      readonly trust: RlmTrustLabelV1;
      readonly origin: RlmContentOriginV1;
      readonly toolName: string;
      readonly intoVar: string;
      readonly kind: "json" | "blob";
    };

export const RlmObservationV1Schema: Schema.Schema<RlmObservationV1> = Schema.Union(
  Schema.Struct({
    _tag: Schema.Literal("PreviewResult"),
    trust: RlmTrustLabelV1Schema,
    origin: RlmContentOriginV1Schema,
    target: RlmTargetV1Schema,
    span: RlmSpanRefV1Schema,
    offset: Schema.Number,
    length: Schema.Number,
    totalChars: Schema.Number,
    truncated: Schema.Boolean,
    text: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("SearchResult"),
    trust: RlmTrustLabelV1Schema,
    origin: RlmContentOriginV1Schema,
    target: RlmTargetV1Schema,
    totalChars: Schema.Number,
    query: Schema.String,
    totalMatches: Schema.Number,
    truncated: Schema.Boolean,
    matches: Schema.Array(
      Schema.Struct({
        index: Schema.Number,
        snippet: Schema.String,
        span: RlmSpanRefV1Schema
      })
    )
  }),
  Schema.Struct({
    _tag: Schema.Literal("LoadResult"),
    intoVar: Schema.String,
    blob: BlobRefSchema
  }),
  Schema.Struct({
    _tag: Schema.Literal("ChunkResult"),
    intoVar: Schema.String,
    chunkCount: Schema.Number,
    chunkChars: Schema.Number,
    overlapChars: Schema.Number
  }),
  Schema.Struct({
    _tag: Schema.Literal("WriteVarResult"),
    name: Schema.String,
    kind: Schema.Literal("json", "blob")
  }),
  Schema.Struct({
    _tag: Schema.Literal("SubLmResult"),
    trust: RlmTrustLabelV1Schema,
    origin: RlmContentOriginV1Schema,
    intoVar: Schema.String,
    blob: BlobRefSchema
  }),
  Schema.Struct({
    _tag: Schema.Literal("ExtractOverChunksResult"),
    intoVar: Schema.String,
    chunkCount: Schema.Number,
    outputsVar: Schema.String
  }),
  Schema.Struct({
    _tag: Schema.Literal("ToolCallResult"),
    trust: RlmTrustLabelV1Schema,
    origin: RlmContentOriginV1Schema,
    toolName: Schema.String,
    intoVar: Schema.String,
    kind: Schema.Literal("json", "blob")
  })
);

export type RlmKernelStep =
  | { readonly _tag: "Continue"; readonly observation: RlmObservationV1 }
  | { readonly _tag: "Final"; readonly output: unknown };

function byteLengthUtf8(text: string): number {
  return new TextEncoder().encode(text).byteLength;
}

function normalizeBoundedInt(n: number | undefined, options: { readonly min: number; readonly max: number }): number {
  const v = typeof n === "number" && Number.isFinite(n) ? Math.floor(n) : options.min;
  return Math.max(options.min, Math.min(options.max, v));
}

function lineRangeFromCharSpan(
  text: string,
  startChar: number,
  endChar: number,
  options?: { readonly maxScanChars?: number | undefined }
): { readonly startLine: number; readonly endLine: number } | null {
  // Computing absolute line numbers is O(endChar). For very large blobs, skip it.
  const maxScanChars = Math.max(0, Math.floor(options?.maxScanChars ?? 200_000));
  if (endChar > maxScanChars) return null;

  let line = 1;
  let startLine = 1;
  for (let i = 0; i < endChar; i++) {
    if (i === startChar) startLine = line;
    if (text.charCodeAt(i) === 10) line++; // '\n'
  }
  const endLine = line;
  return { startLine, endLine };
}

function resolveModelConfig(
  params: DseParams,
  role: "main" | "sub" | "repair" | "judge"
): { readonly modelId?: string; readonly temperature?: number; readonly topP?: number; readonly maxTokens?: number } {
  const base = params.model ?? {};
  const roles = params.modelRoles ?? {};
  const override = (roles as any)[role] ?? {};
  return {
    ...(base.modelId ? { modelId: base.modelId } : {}),
    ...(typeof base.temperature === "number" ? { temperature: base.temperature } : {}),
    ...(typeof base.topP === "number" ? { topP: base.topP } : {}),
    ...(typeof base.maxTokens === "number" ? { maxTokens: base.maxTokens } : {}),
    ...(override.modelId ? { modelId: override.modelId } : {}),
    ...(typeof override.temperature === "number" ? { temperature: override.temperature } : {}),
    ...(typeof override.topP === "number" ? { topP: override.topP } : {}),
    ...(typeof override.maxTokens === "number" ? { maxTokens: override.maxTokens } : {})
  };
}

function policyAllowsTool(params: DseParams, toolName: string): boolean {
  const allowed = params.tools?.allowedToolNames;
  if (!allowed || allowed.length === 0) return false;
  return allowed.includes(toolName);
}

function toolTimeoutMs(params: DseParams, toolName: string, requestedTimeoutMs: number | undefined): number | undefined {
  const policyTimeout = params.tools?.timeoutMsByToolName?.[toolName];
  const req = typeof requestedTimeoutMs === "number" && Number.isFinite(requestedTimeoutMs)
    ? Math.max(0, Math.floor(requestedTimeoutMs))
    : undefined;
  const pol = typeof policyTimeout === "number" && Number.isFinite(policyTimeout)
    ? Math.max(0, Math.floor(policyTimeout))
    : undefined;

  if (pol === undefined) return req;
  if (req === undefined) return pol;
  return Math.min(pol, req);
}

function isBlobRefArray(value: unknown): value is ReadonlyArray<BlobRef> {
  if (!Array.isArray(value)) return false;
  // Fast structural check; full schema decode happens when needed.
  return value.every((v) => v && typeof v === "object" && typeof (v as any).id === "string");
}

function resolveTargetTextWithSource(
  target: RlmTargetV1
): Effect.Effect<
  { readonly text: string; readonly source: RlmSourceV1 },
  RlmKernelError,
  BlobStoreService | VarSpaceService
> {
  return Effect.gen(function* () {
    const blobs = yield* BlobStoreService;
    const vars = yield* VarSpaceService;

    if (target._tag === "Blob") {
      const text = yield* blobs.getText(target.blobId).pipe(
        Effect.catchAll((cause) =>
          Effect.fail(RlmKernelError.make({ message: "BlobStore.getText failed", cause }))
        )
      );
      if (text == null) {
        return yield* Effect.fail(
          RlmKernelError.make({ message: `Missing blob (blobId=${target.blobId})` })
        );
      }
      return { text, source: { _tag: "Blob", blobId: target.blobId } };
    }

    const v = yield* vars.get(target.name).pipe(
      Effect.catchAll((cause: VarSpaceError) =>
        Effect.fail(RlmKernelError.make({ message: "VarSpace.get failed", cause }))
      )
    );
    if (v == null) {
      return yield* Effect.fail(
        RlmKernelError.make({ message: `Missing var (name=${target.name})` })
      );
    }

    if (v._tag === "Blob") {
      const text = yield* blobs.getText(v.blob.id).pipe(
        Effect.catchAll((cause) =>
          Effect.fail(RlmKernelError.make({ message: "BlobStore.getText failed", cause }))
        )
      );
      if (text == null) {
        return yield* Effect.fail(
          RlmKernelError.make({ message: `Missing blob for var (name=${target.name} blobId=${v.blob.id})` })
        );
      }
      return { text, source: { _tag: "VarBlob", name: target.name, blobId: v.blob.id } };
    }

    // Keep JSON values addressable as text (small by VarSpace constraint).
    return { text: canonicalJson(v.value), source: { _tag: "VarJson", name: target.name } };
  });
}

export function executeRlmAction(options: {
  readonly action: RlmActionV1;
  readonly params: DseParams;
  readonly budget: BudgetHandle;
}): Effect.Effect<
  RlmKernelStep,
  RlmKernelError | LmClientError | BudgetExceededError,
  BlobStoreService | VarSpaceService | LmClientService
> {
  return Effect.gen(function* () {
    const vars = yield* VarSpaceService;
    const blobs = yield* BlobStoreService;
    const lm = yield* LmClientService;

    yield* options.budget.checkTime();

    const action = options.action;
    const params = options.params;

    switch (action._tag) {
      case "Preview": {
        const resolved = yield* resolveTargetTextWithSource(action.target);
        const text = resolved.text;
        const offset = normalizeBoundedInt(action.offset, { min: 0, max: Math.max(0, text.length) });
        const length = normalizeBoundedInt(action.length, { min: 1, max: 10_000 });
        const end = Math.min(text.length, offset + length);
        const slice = text.slice(offset, end);
        const truncated = end < text.length;

        const span: RlmSpanRefV1 = {
          _tag: "SpanRef",
          source: resolved.source,
          startChar: offset,
          endChar: end,
          totalChars: text.length,
          ...(lineRangeFromCharSpan(text, offset, end) ?? {})
        };

        return {
          _tag: "Continue",
          observation: {
            _tag: "PreviewResult",
            trust: "untrusted",
            origin: "unknown",
            target: action.target,
            span,
            offset,
            length,
            totalChars: text.length,
            truncated,
            text: slice
          }
        };
      }
      case "Search": {
        const resolved = yield* resolveTargetTextWithSource(action.target);
        const text = resolved.text;
        const query = action.query;
        const maxMatches = normalizeBoundedInt(action.maxMatches, { min: 1, max: 200 });
        const contextChars = normalizeBoundedInt(action.contextChars, { min: 0, max: 500 });

        let i = 0;
        let totalMatches = 0;
        const matches: Array<{ readonly index: number; readonly snippet: string; readonly span: RlmSpanRefV1 }> = [];
        while (true) {
          const idx = text.indexOf(query, i);
          if (idx === -1) break;
          totalMatches++;

          if (matches.length < maxMatches) {
            const start = Math.max(0, idx - contextChars);
            const end = Math.min(text.length, idx + query.length + contextChars);
            const snippet = text.slice(start, end);
            matches.push({
              index: idx,
              snippet,
              span: {
                _tag: "SpanRef",
                source: resolved.source,
                startChar: start,
                endChar: end,
                totalChars: text.length,
                ...(lineRangeFromCharSpan(text, start, end) ?? {})
              }
            });
          }

          i = idx + Math.max(1, query.length);
        }

        return {
          _tag: "Continue",
          observation: {
            _tag: "SearchResult",
            trust: "untrusted",
            origin: "unknown",
            target: action.target,
            totalChars: text.length,
            query,
            totalMatches,
            truncated: totalMatches > matches.length,
            matches
          }
        };
      }
      case "Load": {
        const text = yield* blobs.getText(action.blobId).pipe(
          Effect.catchAll((cause: BlobStoreError) =>
            Effect.fail(RlmKernelError.make({ message: "BlobStore.getText failed", cause }))
          )
        );
        if (text == null) {
          return yield* Effect.fail(
            RlmKernelError.make({ message: `Missing blob (blobId=${action.blobId})` })
          );
        }

        const ref: BlobRef = {
          id: action.blobId,
          hash: action.blobId,
          size: byteLengthUtf8(text)
        };

        yield* vars.putBlob(action.intoVar, ref).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putBlob failed", cause }))
          )
        );

        return {
          _tag: "Continue",
          observation: { _tag: "LoadResult", intoVar: action.intoVar, blob: ref }
        };
      }
      case "Chunk": {
        const resolved = yield* resolveTargetTextWithSource(action.target);
        const text = resolved.text;
        const chunkChars = normalizeBoundedInt(action.chunkChars, { min: 1, max: 20_000 });
        const overlapChars = normalizeBoundedInt(action.overlapChars, { min: 0, max: Math.max(0, chunkChars - 1) });
        const maxChunks = normalizeBoundedInt(action.maxChunks, { min: 1, max: 200 });

        const chunks: Array<BlobRef> = [];
        let start = 0;
        while (start < text.length && chunks.length < maxChunks) {
          const end = Math.min(text.length, start + chunkChars);
          const chunkText = text.slice(start, end);
          const ref = yield* blobs.putText({ text: chunkText, mime: "text/plain" }).pipe(
            Effect.catchAll((cause: BlobStoreError) =>
              Effect.fail(RlmKernelError.make({ message: "BlobStore.putText failed", cause }))
            )
          );
          chunks.push(ref);

          if (end >= text.length) break;
          const nextStart = end - overlapChars;
          // Ensure progress even if overlap is large.
          start = nextStart <= start ? end : nextStart;
        }

        yield* vars.putJson(action.intoVar, chunks).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putJson failed", cause }))
          )
        );

        return {
          _tag: "Continue",
          observation: {
            _tag: "ChunkResult",
            intoVar: action.intoVar,
            chunkCount: chunks.length,
            chunkChars,
            overlapChars
          }
        };
      }
      case "WriteVar": {
        if (action.value._tag === "Json") {
          yield* vars.putJson(action.name, action.value.value).pipe(
            Effect.catchAll((cause: VarSpaceError) =>
              Effect.fail(RlmKernelError.make({ message: "VarSpace.putJson failed", cause }))
            )
          );
          return {
            _tag: "Continue",
            observation: { _tag: "WriteVarResult", name: action.name, kind: "json" }
          };
        }

        yield* vars.putBlob(action.name, action.value.blob).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putBlob failed", cause }))
          )
        );
        return {
          _tag: "Continue",
          observation: { _tag: "WriteVarResult", name: action.name, kind: "blob" }
        };
      }
      case "SubLm": {
        // Sub-LM calls are budgeted (both as LM calls and as sub-LM calls).
        yield* options.budget.onLmCall();
        yield* options.budget.onSubLmCall();

        const subRole = params.rlmLite?.subRole ?? "sub";
        const roleCfg = resolveModelConfig(params, subRole === "main" ? "main" : "sub");

        const response = yield* lm.complete({
          messages: action.messages,
          modelId: action.model?.modelId ?? roleCfg.modelId,
          temperature: action.model?.temperature ?? roleCfg.temperature,
          topP: action.model?.topP ?? roleCfg.topP,
          maxTokens: action.model?.maxTokens ?? Math.min(1024, roleCfg.maxTokens ?? 1024)
        });

        yield* options.budget.onOutputChars(response.text.length);

        const blob = yield* blobs.putText({ text: response.text, mime: "text/plain" }).pipe(
          Effect.catchAll((cause: BlobStoreError) =>
            Effect.fail(RlmKernelError.make({ message: "BlobStore.putText failed", cause }))
          )
        );

        yield* vars.putBlob(action.intoVar, blob).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putBlob failed", cause }))
          )
        );

        return {
          _tag: "Continue",
          observation: { _tag: "SubLmResult", trust: "untrusted", origin: "lm", intoVar: action.intoVar, blob }
        };
      }
      case "ExtractOverChunks": {
        const maxChunks = normalizeBoundedInt(action.maxChunks, { min: 1, max: 200 });

        const vv = yield* vars.get(action.chunksVar).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.get failed", cause }))
          )
        );
        if (vv == null || vv._tag !== "Json") {
          return yield* Effect.fail(
            RlmKernelError.make({
              message: `extract_over_chunks requires a Json var containing a BlobRef[] (name=${action.chunksVar})`
            })
          );
        }
        if (!isBlobRefArray(vv.value)) {
          return yield* Effect.fail(
            RlmKernelError.make({
              message: `extract_over_chunks expected BlobRef[] (name=${action.chunksVar})`
            })
          );
        }

        const chunks0 = vv.value.slice(0, maxChunks);

        // Validate schema strictly so downstream code doesn't crash.
        const chunks = yield* Effect.try({
          try: () => Schema.decodeUnknownSync(Schema.Array(BlobRefSchema))(chunks0),
          catch: (cause) =>
            RlmKernelError.make({
              message: `extract_over_chunks chunk list failed schema decode (name=${action.chunksVar})`,
              cause
            })
        });

        const outputs: Array<BlobRef> = [];
        for (const [idx, chunk] of chunks.entries()) {
          const chunkText = yield* blobs.getText(chunk.id).pipe(
            Effect.catchAll((cause: BlobStoreError) =>
              Effect.fail(RlmKernelError.make({ message: "BlobStore.getText failed", cause }))
            )
          );
          if (chunkText == null) {
            return yield* Effect.fail(
              RlmKernelError.make({
                message: `Missing chunk blob (index=${idx} blobId=${chunk.id})`
              })
            );
          }

          yield* options.budget.onLmCall();
          yield* options.budget.onSubLmCall();

          const subRole = params.rlmLite?.subRole ?? "sub";
          const roleCfg = resolveModelConfig(params, subRole === "main" ? "main" : "sub");
          const extractorSystem = params.rlmLite?.extractionSystem ?? "You are a focused extraction helper.";

          const response = yield* lm.complete({
            messages: [
              { role: "system", content: extractorSystem },
              {
                role: "user",
                content: action.instruction + "\n\nChunk:\n" + chunkText
              }
            ],
            modelId: action.model?.modelId ?? roleCfg.modelId,
            temperature: action.model?.temperature ?? roleCfg.temperature,
            topP: action.model?.topP ?? roleCfg.topP,
            maxTokens: action.model?.maxTokens ?? Math.min(1024, roleCfg.maxTokens ?? 1024)
          });
          yield* options.budget.onOutputChars(response.text.length);

          const outBlob = yield* blobs.putText({ text: response.text, mime: "text/plain" }).pipe(
            Effect.catchAll((cause: BlobStoreError) =>
              Effect.fail(RlmKernelError.make({ message: "BlobStore.putText failed", cause }))
            )
          );
          outputs.push(outBlob);
        }

        yield* vars.putJson(action.intoVar, outputs).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putJson failed", cause }))
          )
        );

        return {
          _tag: "Continue",
          observation: {
            _tag: "ExtractOverChunksResult",
            intoVar: action.intoVar,
            chunkCount: chunks.length,
            outputsVar: action.intoVar
          }
        };
      }
      case "ToolCall": {
        if (!policyAllowsTool(params, action.toolName)) {
          return yield* Effect.fail(
            RlmKernelError.make({
              message: `Tool not allowed by policy (toolName=${action.toolName})`
            })
          );
        }

        yield* options.budget.onToolCall();

        // Enforce params.tools.maxToolCalls (separate from budgets.maxToolCalls).
        const maxToolCalls = params.tools?.maxToolCalls;
        if (typeof maxToolCalls === "number" && Number.isFinite(maxToolCalls)) {
          const snap = yield* options.budget.snapshot();
          const used = snap.usage.toolCalls ?? 0;
          if (used > Math.max(0, Math.floor(maxToolCalls))) {
            return yield* Effect.fail(
              RlmKernelError.make({
                message: `Tool call limit exceeded by policy: maxToolCalls=${maxToolCalls} observed=${used}`
              })
            );
          }
        }

        const toolExecOpt = yield* Effect.serviceOption(ToolExecutorService);
        if (Option.isNone(toolExecOpt)) {
          return yield* Effect.fail(
            RlmKernelError.make({
              message: `Tool executor not configured (toolName=${action.toolName})`
            })
          );
        }

        const timeoutMs = toolTimeoutMs(params, action.toolName, action.timeoutMs);
        const out = yield* toolExecOpt.value
          .call({ toolName: action.toolName, input: action.input, timeoutMs })
          .pipe(
            Effect.catchAll((cause: ToolCallError) =>
              Effect.fail(
                RlmKernelError.make({
                  message: `Tool call failed (toolName=${action.toolName})`,
                  cause
                })
              )
            )
          );

        // Store tool output in VarSpace; spill to blob if too large.
        const rendered = yield* Effect.try({
          try: () => canonicalJson(out),
          catch: (cause) =>
            RlmKernelError.make({ message: "Failed to canonicalize tool output", cause })
        });

        const maxInlineChars = 20_000;
        if (rendered.length <= maxInlineChars) {
          yield* vars.putJson(action.intoVar, out).pipe(
            Effect.catchAll((cause: VarSpaceError) =>
              Effect.fail(RlmKernelError.make({ message: "VarSpace.putJson failed", cause }))
            )
          );
          return {
            _tag: "Continue",
            observation: {
              _tag: "ToolCallResult",
              trust: "untrusted",
              origin: "tool",
              toolName: action.toolName,
              intoVar: action.intoVar,
              kind: "json"
            }
          };
        }

        const blob = yield* blobs.putText({ text: rendered, mime: "application/json" }).pipe(
          Effect.catchAll((cause: BlobStoreError) =>
            Effect.fail(RlmKernelError.make({ message: "BlobStore.putText failed", cause }))
          )
        );
        yield* vars.putBlob(action.intoVar, blob).pipe(
          Effect.catchAll((cause: VarSpaceError) =>
            Effect.fail(RlmKernelError.make({ message: "VarSpace.putBlob failed", cause }))
          )
        );
        return {
          _tag: "Continue",
          observation: {
            _tag: "ToolCallResult",
            trust: "untrusted",
            origin: "tool",
            toolName: action.toolName,
            intoVar: action.intoVar,
            kind: "blob"
          }
        };
      }
      case "Final":
        return { _tag: "Final", output: action.output };
    }
  });
}
