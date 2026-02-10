import { Effect, Schema } from "effect";

import { canonicalJson } from "../internal/canonicalJson.js";
import type { DseParams } from "../params.js";
import type { DseSignature } from "../signature.js";
import type { LmMessage } from "./lm.js";
import type { ContextEntry, PromptBlock, PromptIR } from "../promptIr.js";
import { BlobStoreService } from "./blobStore.js";

export class PromptRenderError extends Schema.TaggedError<PromptRenderError>()(
  "PromptRenderError",
  {
    signatureId: Schema.String,
    message: Schema.String,
    missingExampleIds: Schema.optional(Schema.Array(Schema.String)),
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type PromptRenderBlobStatV1 = {
  readonly blobId: string;
  readonly mime?: string | undefined;
  readonly declaredSize?: number | undefined;
  readonly contentChars: number;
  readonly previewChars: number;
  readonly truncated: boolean;
};

export const PromptRenderBlobStatV1Schema: Schema.Schema<PromptRenderBlobStatV1> =
  Schema.Struct({
    blobId: Schema.String,
    mime: Schema.optional(Schema.String),
    declaredSize: Schema.optional(Schema.Number),
    contentChars: Schema.Number,
    previewChars: Schema.Number,
    truncated: Schema.Boolean
  });

export type PromptRenderStatsV1 = {
  readonly format: "openagents.dse.prompt_render_stats";
  readonly formatVersion: 1;

  readonly messageCount: number;
  readonly totalChars: number;

  readonly byRole: {
    readonly systemChars: number;
    readonly userChars: number;
  };

  readonly context: {
    readonly inlineEntryCount: number;
    readonly inlineValueChars: number;

    readonly blobEntryCount: number;
    readonly blobContentChars: number;
    readonly blobContentPreviewChars: number;

    readonly blobsDropped: number;
    readonly blobs: ReadonlyArray<PromptRenderBlobStatV1>;
  };

  readonly fewShot: {
    readonly exampleCount: number;
    readonly totalChars: number;
  };
};

export const PromptRenderStatsV1Schema: Schema.Schema<PromptRenderStatsV1> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.prompt_render_stats"),
    formatVersion: Schema.Literal(1),
    messageCount: Schema.Number,
    totalChars: Schema.Number,
    byRole: Schema.Struct({
      systemChars: Schema.Number,
      userChars: Schema.Number
    }),
    context: Schema.Struct({
      inlineEntryCount: Schema.Number,
      inlineValueChars: Schema.Number,
      blobEntryCount: Schema.Number,
      blobContentChars: Schema.Number,
      blobContentPreviewChars: Schema.Number,
      blobsDropped: Schema.Number,
      blobs: Schema.Array(PromptRenderBlobStatV1Schema)
    }),
    fewShot: Schema.Struct({
      exampleCount: Schema.Number,
      totalChars: Schema.Number
    })
  });

function applyParamsToPromptIr<I, O>(
  signatureId: string,
  prompt: PromptIR<I, O>,
  params: DseParams
): Effect.Effect<PromptIR<I, O>, PromptRenderError> {
  return Effect.gen(function* () {
    const instructionText = params.instruction?.text;
    const selectedExampleIds = params.fewShot?.exampleIds;
    const tools = params.tools;

    // Precompute example id existence so we can fail fast and deterministically.
    if (selectedExampleIds && selectedExampleIds.length > 0) {
      const allExampleIds = new Set<string>();
      for (const block of prompt.blocks) {
        if (block._tag !== "FewShot") continue;
        for (const ex of block.examples) allExampleIds.add(ex.id);
      }
      const missing = selectedExampleIds.filter((id) => !allExampleIds.has(id));
      if (missing.length > 0) {
        return yield* PromptRenderError.make({
          signatureId,
          message: "Few-shot params referenced unknown example ids",
          missingExampleIds: missing
        });
      }
    }

    let sawToolPolicy = false;
    const nextBlocks: Array<PromptBlock<I, O>> = prompt.blocks.map((block) => {
      switch (block._tag) {
        case "Instruction":
          return instructionText ? { ...block, text: instructionText } : block;
        case "FewShot":
          if (!selectedExampleIds || selectedExampleIds.length === 0) return block;
          // Preserve the authored example order, but only keep selected ids.
          return {
            ...block,
            examples: block.examples.filter((ex) =>
              selectedExampleIds.includes(ex.id)
            )
          };
        case "ToolPolicy": {
          sawToolPolicy = true;
          if (!tools) return block;
          return {
            ...block,
            policy: {
              ...block.policy,
              ...(tools.allowedToolNames
                ? { allowedToolNames: tools.allowedToolNames }
                : {}),
              ...(typeof tools.maxToolCalls === "number"
                ? { maxToolCalls: tools.maxToolCalls }
                : {}),
              ...(tools.timeoutMsByToolName
                ? { timeoutMsByToolName: tools.timeoutMsByToolName }
                : {})
            }
          };
        }
        default:
          return block;
      }
    });

    if (tools && !sawToolPolicy) {
      nextBlocks.push({
        _tag: "ToolPolicy",
        policy: {
          ...(tools.allowedToolNames ? { allowedToolNames: tools.allowedToolNames } : {}),
          ...(typeof tools.maxToolCalls === "number"
            ? { maxToolCalls: tools.maxToolCalls }
            : {}),
          ...(tools.timeoutMsByToolName
            ? { timeoutMsByToolName: tools.timeoutMsByToolName }
            : {})
        }
      });
    }

    return { ...prompt, blocks: nextBlocks };
  });
}

function joinNonEmpty(parts: ReadonlyArray<string>): string {
  return parts.map((p) => p.trim()).filter((p) => p.length > 0).join("\n\n");
}

export function renderPromptMessagesWithStats<I, O>(options: {
  readonly signature: DseSignature<I, O>;
  readonly input: I;
  readonly params: DseParams;
  readonly blobContextMode?: "inline_preview" | "metadata_only" | undefined;
}): Effect.Effect<
  { readonly messages: ReadonlyArray<LmMessage>; readonly stats: PromptRenderStatsV1 },
  PromptRenderError,
  BlobStoreService
> {
  return Effect.gen(function* () {
    const sig = options.signature;
    const blobStore = yield* BlobStoreService;

    const prompt = yield* applyParamsToPromptIr(sig.id, sig.prompt, options.params);

    const encodeInput = Schema.encodeSync(sig.input);
    const encodeOutput = Schema.encodeSync(sig.output);

    const systemText = joinNonEmpty(
      prompt.blocks.flatMap((b) => (b._tag === "System" ? [b.text] : []))
    );

    const instructionText = joinNonEmpty(
      prompt.blocks.flatMap((b) =>
        b._tag === "Instruction" ? [b.text] : []
      )
    );

    let inlineEntryCount = 0;
    let inlineValueChars = 0;
    let blobEntryCount = 0;
    let blobContentChars = 0;
    let blobContentPreviewChars = 0;
    let blobsDropped = 0;

    const MAX_BLOB_STATS = 200;
    const blobs: Array<PromptRenderBlobStatV1> = [];

    const blobContextMode = options.blobContextMode ?? "inline_preview";

    const renderContextEntry = (
      e: ContextEntry
    ): Effect.Effect<string, PromptRenderError> => {
      if ("value" in e) {
        const rendered = canonicalJson(e.value);
        inlineEntryCount++;
        inlineValueChars += rendered.length;
        return Effect.succeed(`${e.key}: ${rendered}`);
      }

      return blobStore.getText(e.blob.id).pipe(
        Effect.flatMap((text) => {
          if (text == null) {
            return Effect.fail(
              PromptRenderError.make({
                signatureId: sig.id,
                message: `Missing blob for context entry (blobId=${e.blob.id})`
              })
            );
          }

          const MAX_CONTEXT_CHARS = 20_000;
          const truncatedFlag = text.length > MAX_CONTEXT_CHARS;
          const previewText =
            blobContextMode === "inline_preview"
              ? truncatedFlag
                ? text.slice(0, MAX_CONTEXT_CHARS) + "\n...[truncated]"
                : text
              : "";

          blobEntryCount++;
          blobContentChars += text.length;
          blobContentPreviewChars += previewText.length;

          if (blobs.length < MAX_BLOB_STATS) {
            blobs.push({
              blobId: e.blob.id,
              ...(e.blob.mime ? { mime: e.blob.mime } : {}),
              ...(typeof e.blob.size === "number" ? { declaredSize: e.blob.size } : {}),
              contentChars: text.length,
              previewChars: previewText.length,
              truncated: truncatedFlag
            });
          } else {
            blobsDropped++;
          }

          const metaParts: Array<string> = [];
          metaParts.push(`blobId=${e.blob.id}`);
          if (e.blob.mime) metaParts.push(`mime=${e.blob.mime}`);
          metaParts.push(`size=${e.blob.size}`);

          if (blobContextMode === "metadata_only") {
            return Effect.succeed(
              `${e.key} (${metaParts.join(" ")}): [blob omitted; use RLM ops to preview/search/chunk]`
            );
          }

          return Effect.succeed(`${e.key} (${metaParts.join(" ")}):\n${previewText}`);
        }),
        Effect.catchAll((cause) =>
          Effect.fail(
            PromptRenderError.make({
              signatureId: sig.id,
              message: "Failed to load blob for context entry",
              cause
            })
          )
        )
      );
    };

    const contextSections = yield* Effect.forEach(
      prompt.blocks,
      (b) => {
        if (b._tag !== "Context") return Effect.succeed("");
        return Effect.forEach(b.entries, renderContextEntry).pipe(
          Effect.map((lines) => (lines.length ? "Context:\n" + lines.join("\n") : ""))
        );
      },
      { discard: false }
    );

    const contextText = joinNonEmpty(contextSections);

    const toolPolicyText = joinNonEmpty(
      prompt.blocks.flatMap((b) => {
        if (b._tag !== "ToolPolicy") return [];
        const p = b.policy;
        const allowed = p.allowedToolNames?.length
          ? `Allowed tools: ${p.allowedToolNames.join(", ")}`
          : "Allowed tools: (none declared)";
        const maxCalls =
          typeof p.maxToolCalls === "number"
            ? `Max tool calls: ${p.maxToolCalls}`
            : "";
        return [joinNonEmpty(["Tools:", allowed, maxCalls])];
      })
    );

    const outputFormatText = joinNonEmpty(
      prompt.blocks.flatMap((b) => {
        if (b._tag !== "OutputFormat") return [];
        switch (b.format._tag) {
          case "JsonOnly":
            return ["Output format: JSON only. No markdown fences, no extra keys."];
          case "JsonSchema":
            return [
              "Output format: JSON only. Match this schema:",
              canonicalJson(b.format.schema)
            ];
        }
      })
    );

    const fewShotBlocks = prompt.blocks.filter((b) => b._tag === "FewShot");
    const fewShotExampleCount = fewShotBlocks.reduce(
      (acc, b) => acc + (b._tag === "FewShot" ? b.examples.length : 0),
      0
    );
    const fewShotSections = yield* Effect.forEach(fewShotBlocks, (b) => {
      if (b._tag !== "FewShot" || b.examples.length === 0) {
        return Effect.succeed("");
      }

      return Effect.forEach(b.examples, (ex) => {
        const encodedIn = Effect.try({
          try: () => encodeInput(ex.input),
          catch: (cause) =>
            PromptRenderError.make({
              signatureId: sig.id,
              message: `Failed to encode few-shot input (exampleId=${ex.id})`,
              cause
            })
        });

        const encodedOut = Effect.try({
          try: () => encodeOutput(ex.output),
          catch: (cause) =>
            PromptRenderError.make({
              signatureId: sig.id,
              message: `Failed to encode few-shot output (exampleId=${ex.id})`,
              cause
            })
        });

        return Effect.all([encodedIn, encodedOut]).pipe(
          Effect.map(([i, o]) => canonicalJson({ id: ex.id, input: i, output: o }))
        );
      }).pipe(Effect.map((lines) => "Few-shot examples:\n" + lines.join("\n")));
    });

    const fewShotText = joinNonEmpty(fewShotSections);
    const fewShotTotalChars = fewShotText.length;

    const encodedInput = yield* Effect.try({
      try: () => encodeInput(options.input),
      catch: (cause) =>
        PromptRenderError.make({
          signatureId: sig.id,
          message: "Failed to encode input",
          cause
        })
    });

    const inputText = "Input:\n" + canonicalJson(encodedInput);

    const userText = joinNonEmpty([
      instructionText,
      contextText,
      toolPolicyText,
      outputFormatText,
      fewShotText,
      inputText
    ]);

    const messages: Array<LmMessage> = [];
    if (systemText) messages.push({ role: "system", content: systemText });
    messages.push({ role: "user", content: userText });

    let systemChars = 0;
    let userChars = 0;
    let totalChars = 0;
    for (const m of messages) {
      const n = m.content.length;
      totalChars += n;
      if (m.role === "system") systemChars += n;
      if (m.role === "user") userChars += n;
    }

    const stats: PromptRenderStatsV1 = {
      format: "openagents.dse.prompt_render_stats",
      formatVersion: 1,
      messageCount: messages.length,
      totalChars,
      byRole: { systemChars, userChars },
      context: {
        inlineEntryCount,
        inlineValueChars,
        blobEntryCount,
        blobContentChars,
        blobContentPreviewChars,
        blobsDropped,
        blobs
      },
      fewShot: {
        exampleCount: fewShotExampleCount,
        totalChars: fewShotTotalChars
      }
    };

    return { messages, stats };
  });
}

export function renderPromptMessages<I, O>(options: {
  readonly signature: DseSignature<I, O>;
  readonly input: I;
  readonly params: DseParams;
}): Effect.Effect<ReadonlyArray<LmMessage>, PromptRenderError, BlobStoreService> {
  return renderPromptMessagesWithStats(options).pipe(
    Effect.map((r) => r.messages)
  );
}
