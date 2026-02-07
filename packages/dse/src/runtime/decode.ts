import { Effect, Schema } from "effect";

export class OutputDecodeError extends Schema.TaggedError<OutputDecodeError>()(
  "OutputDecodeError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect),
    outputPreview: Schema.optional(Schema.String)
  }
) {}

function stripCodeFences(text: string): string {
  const trimmed = text.trim();
  if (!trimmed.startsWith("```")) return trimmed;

  // ```json\n{...}\n```
  const lines = trimmed.split("\n");
  if (lines.length < 2) return trimmed;

  // Drop first fence line.
  const withoutFirst = lines.slice(1).join("\n");
  const endFenceIndex = withoutFirst.lastIndexOf("```");
  if (endFenceIndex === -1) return withoutFirst.trim();

  return withoutFirst.slice(0, endFenceIndex).trim();
}

function extractFirstJsonValue(text: string): string | null {
  const idxObj = text.indexOf("{");
  const idxArr = text.indexOf("[");
  const start =
    idxObj === -1
      ? idxArr
      : idxArr === -1
        ? idxObj
        : Math.min(idxObj, idxArr);

  if (start === -1) return null;

  const open = text[start]!;
  const close = open === "{" ? "}" : "]";

  let depth = 0;
  let inString = false;
  let escape = false;

  for (let i = start; i < text.length; i++) {
    const ch = text[i]!;

    if (inString) {
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === "\\") {
        escape = true;
        continue;
      }
      if (ch === "\"") {
        inString = false;
      }
      continue;
    }

    if (ch === "\"") {
      inString = true;
      continue;
    }

    if (ch === open) depth++;
    if (ch === close) depth--;

    if (depth === 0) {
      return text.slice(start, i + 1);
    }
  }

  return null;
}

function parseStrictJson(stripped: string): unknown {
  return JSON.parse(stripped) as unknown;
}

function parseJsonish(stripped: string): unknown {
  const extracted = extractFirstJsonValue(stripped);
  if (!extracted) {
    throw new Error("No JSON object/array found in output");
  }
  return JSON.parse(extracted) as unknown;
}

export function decodeJsonOutput<O>(
  schema: Schema.Schema<O>,
  rawText: string
): Effect.Effect<O, OutputDecodeError> {
  return decodeJsonOutputWithMode(schema, rawText, { mode: "strict_json" });
}

export function decodeJsonOutputWithMode<O>(
  schema: Schema.Schema<O>,
  rawText: string,
  options: { readonly mode: "strict_json" | "jsonish" }
): Effect.Effect<O, OutputDecodeError> {
  return Effect.gen(function* () {
    const stripped = stripCodeFences(rawText);

    const parsed = yield* Effect.try({
      try: () => {
        try {
          return parseStrictJson(stripped);
        } catch (error) {
          if (options.mode !== "jsonish") throw error;
          return parseJsonish(stripped);
        }
      },
      catch: (cause) =>
        OutputDecodeError.make({
          message: "Failed to parse JSON output",
          cause,
          outputPreview: rawText.slice(0, 500)
        })
    });

    return yield* Effect.try({
      try: () => Schema.decodeUnknownSync(schema)(parsed),
      catch: (cause) =>
        OutputDecodeError.make({
          message: "Output did not match schema",
          cause,
          outputPreview: rawText.slice(0, 500)
        })
    });
  });
}
