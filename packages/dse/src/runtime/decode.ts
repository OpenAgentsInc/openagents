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

export function decodeJsonOutput<O>(
  schema: Schema.Schema<O>,
  rawText: string
): Effect.Effect<O, OutputDecodeError> {
  return Effect.gen(function* () {
    const stripped = stripCodeFences(rawText);

    const parsed = yield* Effect.try({
      try: () => JSON.parse(stripped) as unknown,
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

