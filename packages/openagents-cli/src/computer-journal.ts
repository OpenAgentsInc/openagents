import { Effect, Layer, Schema } from "effect";
import * as Context from "effect/Context";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

import { ConfigurationError } from "./errors.js";
import { ComputerConfiguration } from "./computer-config.js";

export interface JournalEntry {
  readonly at: string;
  readonly requestId: string;
  readonly argv: ReadonlyArray<string>;
  readonly cwd: string;
  readonly decision: string;
  readonly outcome: string;
  readonly detail: string;
}

const StoredEntry = Schema.Struct({
  at: Schema.String,
  requestId: Schema.String,
  argv: Schema.Array(Schema.String),
  cwd: Schema.String,
  decision: Schema.String,
  outcome: Schema.String,
  detail: Schema.String,
});
const decodeStoredEntry = Schema.decodeUnknownEffect(StoredEntry);

export interface JournalInterface {
  readonly append: (entry: Omit<JournalEntry, "at">) => Effect.Effect<void, ConfigurationError>;
  readonly read: (limit: number) => Effect.Effect<ReadonlyArray<JournalEntry>, ConfigurationError>;
}

export class ComputerJournal extends Context.Service<ComputerJournal, JournalInterface>()(
  "@openagentsinc/cli/ComputerJournal",
) {}

export const journalMaxBytes = 1 * 1_024 * 1_024;
export const journalReadTailBytes = 256 * 1_024;

const errorCode = (cause: unknown): string | undefined =>
  typeof cause === "object" && cause !== null && "code" in cause && typeof cause.code === "string"
    ? cause.code
    : undefined;

const redact = (value: string): string =>
  value
    .replaceAll(
      /(?:oa_(?:pat|agent|assignment)_[A-Za-z0-9._-]+|smct_[A-Za-z0-9._-]+)/gu,
      "[REDACTED]",
    )
    .replaceAll(/Bearer\s+\S+/giu, "Bearer [REDACTED]");

const boundedEntry = (entry: Omit<JournalEntry, "at">): Omit<JournalEntry, "at"> => ({
  requestId: redact(entry.requestId).slice(0, 64),
  argv: entry.argv.slice(0, 8).map((value) => redact(value).slice(0, 128)),
  cwd: redact(entry.cwd).slice(0, 1_024),
  decision: redact(entry.decision).slice(0, 64),
  outcome: redact(entry.outcome).slice(0, 64),
  detail: redact(entry.detail).slice(0, 512),
});

const makeJournal = (path: string): JournalInterface => ({
  append: (entry) =>
    Effect.try({
      try: () => {
        const value = { at: new Date().toISOString(), ...boundedEntry(entry) };
        const line = JSON.stringify(value);
        const lineBytes = Buffer.from(`${line}\n`, "utf8");
        mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
        let existing = Buffer.alloc(0);
        try {
          existing = readFileSync(path);
        } catch (cause) {
          if (errorCode(cause) !== "ENOENT") throw cause;
        }
        const retainedLimit = Math.max(0, journalMaxBytes - lineBytes.byteLength);
        const retained =
          existing.byteLength <= retainedLimit
            ? existing
            : (() => {
                const tail = existing.subarray(existing.byteLength - retainedLimit);
                const newline = tail.indexOf(0x0a);
                return newline === -1 ? Buffer.alloc(0) : tail.subarray(newline + 1);
              })();
        writeFileSync(path, Buffer.concat([retained, lineBytes]), { mode: 0o600 });
        chmodSync(path, 0o600);
      },
      catch: () =>
        new ConfigurationError({ message: "The local Computer journal could not be written." }),
    }),
  read: (limit) =>
    Effect.gen(function* () {
      if (limit <= 0) return [];
      const contents = yield* Effect.sync(() => {
        try {
          return { kind: "Read" as const, value: readFileSync(path, "utf8") };
        } catch (cause) {
          return errorCode(cause) === "ENOENT"
            ? { kind: "Missing" as const }
            : { kind: "Error" as const };
        }
      });
      if (contents.kind === "Error") {
        return yield* new ConfigurationError({
          message: "The local Computer journal could not be read.",
        });
      }
      if (contents.kind === "Missing") return [];
      const lines = contents.value
        .slice(-journalReadTailBytes)
        .split("\n")
        .filter((line) => line.trim() !== "")
        .slice(-limit);
      const entries: Array<JournalEntry> = [];
      for (const line of lines) {
        const parsed = yield* Effect.try({
          try: () => JSON.parse(line) as unknown,
          catch: () =>
            new ConfigurationError({
              message: "The local Computer journal contains invalid JSON.",
            }),
        }).pipe(Effect.orElseSucceed(() => undefined));
        if (parsed === undefined) continue;
        const decoded = yield* decodeStoredEntry(parsed).pipe(
          Effect.orElseSucceed(() => undefined),
        );
        if (decoded !== undefined) entries.push(decoded);
      }
      return entries;
    }),
});

export const computerJournalLayer = Layer.effect(
  ComputerJournal,
  Effect.gen(function* () {
    const config = yield* ComputerConfiguration;
    return ComputerJournal.of(makeJournal(config.paths.journal));
  }),
);
