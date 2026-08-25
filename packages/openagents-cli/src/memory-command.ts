/**
 * CLI command definitions for `openagents memory`.
 *
 * Three commands, because the store has three operations. There is no `edit`:
 * a correction is `add --supersedes <id>`, which leaves the memory that was
 * wrong readable behind the one that replaced it.
 *
 * Nothing here recalls anything. Recall runs server-side inside
 * `POST /api/v1/responses`, so a memory written by `add` reaches the next turn
 * with no client plumbing at all (OpenAgentsInc/openagents#51).
 */

import { Effect, Option } from "effect";
import { Argument, Command, Flag } from "effect/unstable/cli";

import { type EndpointOverrides, type Profile } from "./endpoint.js";
import { InputError } from "./errors.js";
import { type MemoryBucket, MemoryClient, type MemoryRecord } from "./memory-client.js";
import { Output, type OutputMode } from "./output.js";
import { resolveApiSession } from "./session.js";

interface SharedFlags {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
  readonly json: boolean;
  readonly noColor: boolean;
}

const endpointOverrides = (flags: {
  readonly profile: Option.Option<Profile>;
  readonly apiUrl: Option.Option<string>;
}): EndpointOverrides => ({ profile: flags.profile, apiUrl: flags.apiUrl });

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

const bucketFlag = Flag.string("bucket").pipe(
  Flag.optional,
  Flag.withDescription("Narrow to one bucket: user or learned"),
);

const limitFlag = Flag.integer("limit").pipe(
  Flag.optional,
  Flag.withDescription("Maximum number of memories to read"),
);

const includeSupersededFlag = Flag.boolean("include-superseded").pipe(
  Flag.withDescription("Also read the corrections behind the live memories"),
);

const supersedesFlag = Flag.string("supersedes").pipe(
  Flag.optional,
  Flag.withDescription("ID of the memory this one corrects and replaces"),
);

const sourceRefFlag = Flag.string("source-ref").pipe(
  Flag.optional,
  Flag.withDescription("Thread or session this memory came out of"),
);

const memoryIdArgument = Argument.string("memory_id").pipe(Argument.withDescription("Memory ID"));

/**
 * Reads a bucket name, or `undefined` for one the server would reject.
 *
 * Checked here rather than left to the API so a typo costs a sentence instead
 * of a round trip that comes back as a validation envelope.
 */
const readBucket = (raw: string): MemoryBucket | undefined => {
  const value = raw.trim().toLowerCase();
  return value === "learned" || value === "user" ? value : undefined;
};

/** The refusal a rejected `--bucket` earns, named for the value that was given. */
const badBucket = (raw: string): InputError =>
  new InputError({ message: `--bucket must be "user" or "learned", not "${raw}".` });

const memoryListHuman = (memories: ReadonlyArray<MemoryRecord>): ReadonlyArray<string> => {
  if (memories.length === 0) return ["No memories stored for this account."];
  const lines: string[] = [];
  for (const memory of memories) {
    // One memory per block rather than one per row: a memory is a sentence a
    // person wrote, and a column would cut most of them off.
    lines.push(`${memory.id}  [${memory.bucket}]  ${memory.created_at}`);
    lines.push(`  ${memory.body}`);
    if (memory.source_ref !== null) lines.push(`  source: ${memory.source_ref}`);
    if (memory.superseded_by !== null) {
      lines.push(`  superseded by: ${memory.superseded_by}`);
    }
  }
  return lines;
};

export const makeMemoryCommand = <R>(root: Effect.Effect<SharedFlags, never, R>) => {
  const memoryListCommand = Command.make(
    "list",
    { bucket: bucketFlag, limit: limitFlag, includeSuperseded: includeSupersededFlag },
    ({ bucket, includeSuperseded, limit }) =>
      Effect.gen(function* () {
        const parsedBucket = Option.isNone(bucket) ? undefined : readBucket(bucket.value);
        if (Option.isSome(bucket) && parsedBucket === undefined) {
          return yield* badBucket(bucket.value);
        }
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* MemoryClient;
        const output = yield* Output;
        const memories = yield* client.list({
          origin: session.endpoint.origin,
          token: session.token,
          ...(parsedBucket === undefined ? {} : { bucket: parsedBucket }),
          ...(Option.isNone(limit) ? {} : { limit: limit.value }),
          ...(includeSuperseded ? { includeSuperseded: true } : {}),
        });
        yield* output.write(
          {
            value: { memories },
            human: memoryListHuman(memories),
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("List the account's memories, newest first"));

  const memoryAddCommand = Command.make(
    "add",
    {
      bucket: bucketFlag,
      supersedes: supersedesFlag,
      sourceRef: sourceRefFlag,
      body: Argument.string("body").pipe(
        Argument.withDescription("What to remember"),
        Argument.variadic({ min: 1 }),
      ),
    },
    ({ body, bucket, sourceRef, supersedes }) =>
      Effect.gen(function* () {
        const parsedBucket = Option.isNone(bucket) ? undefined : readBucket(bucket.value);
        if (Option.isSome(bucket) && parsedBucket === undefined) {
          return yield* badBucket(bucket.value);
        }
        const text = body.join(" ").trim();
        if (text.length === 0) {
          return yield* new InputError({ message: "A memory needs a body to store." });
        }
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* MemoryClient;
        const output = yield* Output;
        const memory = yield* client.create({
          origin: session.endpoint.origin,
          token: session.token,
          body: text,
          ...(parsedBucket === undefined ? {} : { bucket: parsedBucket }),
          ...(Option.isNone(supersedes) ? {} : { supersedes: supersedes.value }),
          ...(Option.isNone(sourceRef) ? {} : { sourceRef: sourceRef.value }),
        });
        yield* output.write(
          {
            value: { memory },
            human: [
              `Stored memory ${memory.id} in the ${memory.bucket} bucket.`,
              `  ${memory.body}`,
              ...(Option.isNone(supersedes) ? [] : [`Supersedes ${supersedes.value}.`]),
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(
    Command.withDescription(
      "Store one memory. Pass --supersedes <id> to correct an existing one rather than edit it",
    ),
  );

  const memoryDeleteCommand = Command.make(
    "delete",
    { memoryId: memoryIdArgument },
    ({ memoryId }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const session = yield* resolveApiSession(endpointOverrides(flags));
        const client = yield* MemoryClient;
        const output = yield* Output;
        const memory = yield* client.remove({
          origin: session.endpoint.origin,
          token: session.token,
          memoryId,
        });
        yield* output.write(
          {
            value: { memory },
            human: [`Removed memory ${memory.id}.`, `  ${memory.body}`],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(Command.withDescription("Remove one memory outright"));

  return Command.make("memory").pipe(
    Command.withDescription(
      "Read and write the account's cloud memories. Recall is server-side; these commands do not retrieve",
    ),
    Command.withSubcommands([memoryListCommand, memoryAddCommand, memoryDeleteCommand]),
  );
};
