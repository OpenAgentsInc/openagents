import { Effect, Layer, Stdio, Stream } from "effect";
import * as Context from "effect/Context";
import { readFile } from "node:fs/promises";

import { InputError } from "./errors.js";

export const STANDARD_INPUT_REFERENCE = "-";

/** A request body larger than this is a mistake rather than an API call. */
export const MAXIMUM_REQUEST_BODY_BYTES = 1_048_576;

export interface RequestBodyInputInterface {
  /** Reads a whole request body from a file path, or from `-` for stdin. */
  readonly read: (reference: string) => Effect.Effect<string, InputError>;
}

export class RequestBodyInput extends Context.Service<
  RequestBodyInput,
  RequestBodyInputInterface
>()("@openagentsinc/cli/RequestBodyInput") {}

const tooLarge = (source: string) =>
  new InputError({
    message: `${source} exceeds ${MAXIMUM_REQUEST_BODY_BYTES} bytes.`,
  });

export const requestBodyInputLayer = Layer.effect(
  RequestBodyInput,
  Effect.gen(function* () {
    const stdio = yield* Stdio.Stdio;

    const readStandardInput = Effect.fn("RequestBodyInput.readStandardInput")(function* () {
      const chunks = yield* Stream.runCollect(stdio.stdin).pipe(
        Effect.mapError(
          () =>
            new InputError({
              message: "The CLI could not read a request body from standard input.",
            }),
        ),
      );
      const byteLength = chunks.reduce((length, chunk) => length + chunk.byteLength, 0);
      if (byteLength > MAXIMUM_REQUEST_BODY_BYTES) {
        return yield* tooLarge("The request body on standard input");
      }
      const bytes = new Uint8Array(byteLength);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      return new TextDecoder().decode(bytes);
    });

    const readPath = Effect.fn("RequestBodyInput.readPath")(function* (path: string) {
      const contents = yield* Effect.tryPromise({
        try: () => readFile(path, { encoding: "utf8" }),
        catch: () => new InputError({ message: `The CLI could not read the file ${path}.` }),
      });
      if (Buffer.byteLength(contents, "utf8") > MAXIMUM_REQUEST_BODY_BYTES) {
        return yield* tooLarge(`The file ${path}`);
      }
      return contents;
    });

    const read = Effect.fn("RequestBodyInput.read")(function* (reference: string) {
      return reference === STANDARD_INPUT_REFERENCE
        ? yield* readStandardInput()
        : yield* readPath(reference);
    });

    return RequestBodyInput.of({ read });
  }),
);

export const requestBodyInputTestLayer = (
  contents: Readonly<Record<string, string>>,
): Layer.Layer<RequestBodyInput> =>
  Layer.succeed(
    RequestBodyInput,
    RequestBodyInput.of({
      read: Effect.fn("RequestBodyInput.Test.read")(function* (reference: string) {
        const value = contents[reference];
        if (value === undefined) {
          return yield* new InputError({
            message: `The CLI could not read the file ${reference}.`,
          });
        }
        return value;
      }),
    }),
  );
