/**
 * The client for the account's cloud memories.
 *
 * Memories live in the openagents.com database, account-scoped, not in a file
 * on this machine (OpenAgentsInc/openagents#51). That decision is what makes
 * this a client at all: the CLI, the web app, and a direct API caller write to
 * one store, and recall happens server-side inside `POST /api/v1/responses`,
 * so nothing here retrieves anything. These three calls — write one, read them
 * back, remove one — are the whole client surface.
 *
 * There is no update. A correction is a new memory carrying `supersedes`, so
 * the store keeps the chain a wrong memory was corrected through rather than
 * overwriting the row that was wrong.
 */

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { MEMORIES_PATH } from "./constants.js";
import type { CliError } from "./errors.js";
import type { AuthenticatedApi } from "./repository-client.js";
import { asRecord, asRows, asText, makeTrackerRequest } from "./tracker-request.js";

/** The two buckets the server accepts. `user` is what a reader asks for. */
export type MemoryBucket = "learned" | "user";

export interface MemoryRecord {
  readonly id: string;
  readonly bucket: string;
  readonly body: string;
  /** The thread or session the request came out of, when one was named. */
  readonly source_ref: string | null;
  /** The id of the memory that replaced this one, once one has. */
  readonly superseded_by: string | null;
  readonly created_at: string;
}

export interface MemoryListInput extends AuthenticatedApi {
  readonly bucket?: MemoryBucket;
  readonly limit?: number;
  /** Reads the corrections behind the live rows as well. */
  readonly includeSuperseded?: boolean;
}

export interface MemoryCreateInput extends AuthenticatedApi {
  readonly body: string;
  readonly bucket?: MemoryBucket;
  readonly sourceRef?: string;
  /** The id of a live memory of this account that this one replaces. */
  readonly supersedes?: string;
}

export interface MemoryDeleteInput extends AuthenticatedApi {
  readonly memoryId: string;
}

export interface MemoryClientInterface {
  readonly list: (input: MemoryListInput) => Effect.Effect<ReadonlyArray<MemoryRecord>, CliError>;
  readonly create: (input: MemoryCreateInput) => Effect.Effect<MemoryRecord, CliError>;
  readonly remove: (input: MemoryDeleteInput) => Effect.Effect<MemoryRecord, CliError>;
}

export class MemoryClient extends Context.Service<MemoryClient, MemoryClientInterface>()(
  "@openagentsinc/cli/MemoryClient",
) {}

/** Reads one row of the server's memory view. */
const parseMemory = (row: Record<string, unknown>): MemoryRecord => ({
  id: asText(row["id"]) ?? "",
  bucket: asText(row["bucket"]) ?? "user",
  body: asText(row["body"]) ?? "",
  source_ref: asText(row["source_ref"]) ?? null,
  superseded_by: asText(row["superseded_by"]) ?? null,
  created_at: asText(row["created_at"]) ?? "",
});

export const memoryClientLayer = Layer.effect(
  MemoryClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;
    const request = makeTrackerRequest(transport);

    const list = Effect.fn("MemoryClient.list")(function* (input: MemoryListInput) {
      const query = new URLSearchParams();
      if (input.bucket !== undefined) query.set("bucket", input.bucket);
      if (input.limit !== undefined) query.set("limit", String(input.limit));
      // The flag is only ever sent as `true`. Its absence is the default, and
      // sending `false` would read as a narrowing the server does not define.
      if (input.includeSuperseded === true) query.set("include_superseded", "true");
      const suffix = query.size === 0 ? "" : `?${query.toString()}`;
      const body = yield* request("list memories", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${MEMORIES_PATH}${suffix}`,
        acceptedStatuses: [200],
      });
      return asRows(body, "memories").map(parseMemory);
    });

    const create = Effect.fn("MemoryClient.create")(function* (input: MemoryCreateInput) {
      const body = yield* request("write memory", {
        origin: input.origin,
        token: input.token,
        method: "POST",
        path: MEMORIES_PATH,
        body: {
          body: input.body,
          // The server defaults an absent bucket to `user`, but a write path
          // that names its bucket keeps working if that default ever moves.
          bucket: input.bucket ?? "user",
          ...(input.sourceRef === undefined ? {} : { source_ref: input.sourceRef }),
          ...(input.supersedes === undefined ? {} : { supersedes: input.supersedes }),
        },
        acceptedStatuses: [201],
      });
      return parseMemory(asRecord(asRecord(body)["memory"]));
    });

    const remove = Effect.fn("MemoryClient.remove")(function* (input: MemoryDeleteInput) {
      const body = yield* request("remove memory", {
        origin: input.origin,
        token: input.token,
        method: "DELETE",
        path: `${MEMORIES_PATH}/${encodeURIComponent(input.memoryId)}`,
        acceptedStatuses: [200],
      });
      return parseMemory(asRecord(asRecord(body)["memory"]));
    });

    return MemoryClient.of({ list, create, remove });
  }),
);
