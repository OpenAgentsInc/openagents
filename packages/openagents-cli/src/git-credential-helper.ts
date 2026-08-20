import { Effect, Option, Redacted, Stdio, Stream } from "effect";

import { CredentialStore } from "./credential-store.js";
import { EnvironmentConfiguration } from "./environment.js";
import { InputError, OutputError } from "./errors.js";

export type GitCredentialOperation = "get" | "store" | "erase";

const readInput = Effect.fn("GitCredentialHelper.readInput")(function* () {
  const stdio = yield* Stdio.Stdio;
  const chunks = yield* Stream.runCollect(stdio.stdin).pipe(
    Effect.mapError(
      () => new InputError({ message: "The credential helper could not read Git input." }),
    ),
  );
  const size = chunks.reduce((total, chunk) => total + chunk.byteLength, 0);
  if (size > 8_192) {
    return yield* new InputError({ message: "The Git credential request exceeded 8192 bytes." });
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
});

export const parseGitCredentialRequest = (input: string): Readonly<Record<string, string>> => {
  const fields: Record<string, string> = {};
  for (const line of input.split(/\r?\n/u)) {
    const separator = line.indexOf("=");
    if (separator <= 0) continue;
    const key = line.slice(0, separator);
    const value = line.slice(separator + 1);
    if (["protocol", "host", "path"].includes(key)) fields[key] = value;
  }
  return fields;
};

const admittedRequest = (origin: string, fields: Readonly<Record<string, string>>): boolean => {
  const target = new URL(origin);
  return fields.protocol === target.protocol.slice(0, -1) && fields.host === target.host;
};

const writeCredential = Effect.fn("GitCredentialHelper.writeCredential")(function* (
  token: Redacted.Redacted<string>,
) {
  const stdio = yield* Stdio.Stdio;
  yield* Stream.make(`username=openagents\npassword=${Redacted.value(token)}\n\n`).pipe(
    Stream.run(stdio.stdout()),
    Effect.mapError(
      (cause) => new OutputError({ message: "The credential helper could not answer Git.", cause }),
    ),
  );
});

export const runGitCredentialHelper = Effect.fn("GitCredentialHelper.run")(function* (
  origin: string,
  operation: GitCredentialOperation,
) {
  const fields = parseGitCredentialRequest(yield* readInput());
  if (!admittedRequest(origin, fields)) return;

  const credentials = yield* CredentialStore;
  if (operation === "erase") {
    yield* credentials.remove(origin);
    return;
  }
  if (operation === "store") return;

  const environment = yield* EnvironmentConfiguration;
  if (Option.isSome(environment.token)) {
    yield* writeCredential(environment.token.value);
    return;
  }

  const stored = yield* credentials.get(origin);
  if (Option.isSome(stored)) yield* writeCredential(stored.value);
});

export const credentialHelperCommand = (origin: string): string =>
  `!openagents --api-url ${origin} auth git-credential`;
