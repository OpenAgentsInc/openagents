import { Effect, Layer, Stdio, Stream } from "effect";
import * as Context from "effect/Context";

import { InputError } from "./errors.js";

export interface SecretInputInterface {
  readonly readToken: () => Effect.Effect<string, InputError>;
}

export class SecretInput extends Context.Service<SecretInput, SecretInputInterface>()(
  "@openagentsinc/cli/SecretInput",
) {}

export const secretInputLayer = Layer.effect(
  SecretInput,
  Effect.gen(function* () {
    const stdio = yield* Stdio.Stdio;
    const readToken = Effect.fn("SecretInput.readToken")(function* () {
      const chunks = yield* Stream.runCollect(stdio.stdin).pipe(
        Effect.mapError(
          () => new InputError({ message: "The CLI could not read a token from standard input." }),
        ),
      );
      const byteLength = chunks.reduce((length, chunk) => length + chunk.byteLength, 0);
      const bytes = new Uint8Array(byteLength);
      let offset = 0;
      for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      const token = new TextDecoder().decode(bytes).trim();
      if (token.length === 0) {
        return yield* new InputError({ message: "No token was provided on standard input." });
      }
      return token;
    });
    return SecretInput.of({ readToken });
  }),
);

export const secretInputTestLayer = (token: string): Layer.Layer<SecretInput> =>
  Layer.succeed(
    SecretInput,
    SecretInput.of({
      readToken: Effect.fn("SecretInput.Test.readToken")(() => Effect.succeed(token)),
    }),
  );
