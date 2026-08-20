import { Effect, Layer, Schema, Stdio, Stream } from "effect";
import * as Context from "effect/Context";

import { OutputError } from "./errors.js";

export type OutputMode = "human" | "json";

export interface OutputDocument {
  readonly value: unknown;
  readonly human: ReadonlyArray<string>;
}

export interface OutputInterface {
  readonly write: (document: OutputDocument, mode: OutputMode) => Effect.Effect<void, OutputError>;
}

export class Output extends Context.Service<Output, OutputInterface>()(
  "@openagentsinc/cli/Output",
) {}

const encodeJson = Effect.fn("Output.encodeJson")(function* (value: unknown) {
  const json = yield* Schema.decodeUnknownEffect(Schema.Json)(value).pipe(
    Effect.mapError(
      (cause) =>
        new OutputError({
          message: "The command result could not be encoded as JSON.",
          cause,
        }),
    ),
  );
  return yield* Schema.encodeEffect(Schema.fromJsonString(Schema.Json))(json).pipe(
    Effect.mapError(
      (cause) =>
        new OutputError({
          message: "The command result could not be encoded as JSON.",
          cause,
        }),
    ),
  );
});

export const outputLayer = Layer.effect(
  Output,
  Effect.gen(function* () {
    const stdio = yield* Stdio.Stdio;
    const write = Effect.fn("Output.write")(function* (document: OutputDocument, mode: OutputMode) {
      const text = mode === "json" ? yield* encodeJson(document.value) : document.human.join("\n");
      yield* Stream.make(`${text}\n`).pipe(
        Stream.run(stdio.stdout()),
        Effect.mapError(
          (cause) =>
            new OutputError({
              message: "The CLI could not write its output.",
              cause,
            }),
        ),
      );
    });
    return Output.of({ write });
  }),
);

export const outputTestLayer = (write: OutputInterface["write"]): Layer.Layer<Output> =>
  Layer.succeed(Output, Output.of({ write }));
