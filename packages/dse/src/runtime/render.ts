import { Effect, Schema } from "effect";

import { canonicalJson } from "../internal/canonicalJson.js";
import type { DseParams } from "../params.js";
import type { DseSignature } from "../signature.js";
import type { LmMessage } from "./lm.js";
import type { PromptBlock, PromptIR } from "../promptIr.js";

export class PromptRenderError extends Schema.TaggedError<PromptRenderError>()(
  "PromptRenderError",
  {
    signatureId: Schema.String,
    message: Schema.String,
    missingExampleIds: Schema.optional(Schema.Array(Schema.String)),
    cause: Schema.optional(Schema.Defect)
  }
) {}

function applyParamsToPromptIr<I, O>(
  signatureId: string,
  prompt: PromptIR<I, O>,
  params: DseParams
): Effect.Effect<PromptIR<I, O>, PromptRenderError> {
  return Effect.gen(function* () {
    const instructionText = params.instruction?.text;
    const selectedExampleIds = params.fewShot?.exampleIds;

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
        default:
          return block;
      }
    });

    return { ...prompt, blocks: nextBlocks };
  });
}

function joinNonEmpty(parts: ReadonlyArray<string>): string {
  return parts.map((p) => p.trim()).filter((p) => p.length > 0).join("\n\n");
}

export function renderPromptMessages<I, O>(options: {
  readonly signature: DseSignature<I, O>;
  readonly input: I;
  readonly params: DseParams;
}): Effect.Effect<ReadonlyArray<LmMessage>, PromptRenderError> {
  return Effect.gen(function* () {
    const sig = options.signature;

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

    const contextText = joinNonEmpty(
      prompt.blocks.flatMap((b) => {
        if (b._tag !== "Context") return [];
        const lines = b.entries.map((e) => `${e.key}: ${canonicalJson(e.value)}`);
        return lines.length ? ["Context:\n" + lines.join("\n")] : [];
      })
    );

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

    return messages;
  });
}
