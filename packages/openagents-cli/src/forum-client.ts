/**
 * The forum API client.
 *
 * Reconstructed from the compiled artifacts of the build that produced the
 * published package, because the original source was never committed. The
 * behavior here is the published behavior: every request path, method, body
 * key, and error branch is taken from `dist/forum-client.js`, and every type is
 * taken from the emitted `dist/forum-client.d.ts`. Neither sourcemap carries
 * `sourcesContent`, so the original text is not recoverable; this file restores
 * what the artifacts prove and nothing more.
 *
 * See issue #153. The published `@openagentsinc/cli` still cannot be rebuilt
 * from committed source until the version constant and the release check land.
 */

import { Effect, Layer, Redacted } from "effect";
import * as Context from "effect/Context";

import { ApiTransport, type ApiRequest } from "./api-transport.js";
import { API_VERSION_PATH } from "./constants.js";
import { ApiError, type CliError } from "./errors.js";

/** An origin and the token that authorizes a request against it. */
export interface AuthenticatedApi {
  readonly origin: string;
  readonly token: Redacted.Redacted<string>;
}

interface ForumClientInterface {
  readonly boards: (input: AuthenticatedApi) => Effect.Effect<unknown, CliError>;
  readonly topics: (
    input: AuthenticatedApi & { readonly board: string; readonly page?: number },
  ) => Effect.Effect<unknown, CliError>;
  readonly search: (
    input: AuthenticatedApi & {
      readonly query: string;
      readonly board?: string;
      readonly page?: number;
    },
  ) => Effect.Effect<unknown, CliError>;
  readonly topic: (
    input: AuthenticatedApi & { readonly id: string; readonly page?: number },
  ) => Effect.Effect<unknown, CliError>;
  readonly createTopic: (
    input: AuthenticatedApi & {
      readonly board: string;
      readonly title: string;
      readonly bodyText: string;
    },
  ) => Effect.Effect<unknown, CliError>;
  readonly reply: (
    input: AuthenticatedApi & { readonly topicId: string; readonly bodyText: string },
  ) => Effect.Effect<unknown, CliError>;
  readonly claim: (
    input: AuthenticatedApi & { readonly actorRef: string },
  ) => Effect.Effect<unknown, CliError>;
  readonly claims: (input: AuthenticatedApi) => Effect.Effect<unknown, CliError>;
}

export class ForumClient extends Context.Service<ForumClient, ForumClientInterface>()(
  "@openagentsinc/cli/ForumClient",
) {}

/** Render a field's messages, which the API may send as a list or a string. */
const messageList = (value: unknown): string =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string").join(", ")
    : typeof value === "string"
      ? value
      : JSON.stringify(value);

/**
 * Turn a failure body into a code and a message.
 *
 * The forum routes predate the unified error envelope, so this reads the three
 * shapes they actually return: a bare string, `{"error": "..."}`, and a
 * field-to-messages map. A body in none of those shapes reports the status
 * rather than guessing.
 */
const errorMessage = (
  body: unknown,
  status: number,
): { readonly code?: string; readonly message: string } => {
  if (typeof body === "string" && body.trim() !== "") {
    return { message: body.trim().slice(0, 200) };
  }

  if (body !== null && typeof body === "object") {
    const record = body as Record<string, unknown>;

    if (typeof record.error === "string") {
      return { code: record.error, message: record.error };
    }

    const errors = record.errors;
    if (errors !== null && typeof errors === "object") {
      return {
        message: Object.entries(errors as Record<string, unknown>)
          .map(([field, messages]) => `${field} ${messageList(messages)}`)
          .join("; "),
      };
    }
  }

  return { message: `The forum API returned HTTP ${status}.` };
};

export const forumClientLayer = Layer.effect(
  ForumClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;

    const request = (
      operation: string,
      input: AuthenticatedApi & {
        readonly method: ApiRequest["method"];
        readonly path: string;
        readonly body?: unknown;
      },
    ): Effect.Effect<unknown, CliError> =>
      Effect.gen(function* () {
        const response = yield* transport.request({
          origin: input.origin,
          method: input.method,
          path: input.path,
          token: input.token,
          ...(input.body === undefined ? {} : { body: input.body }),
        });

        if (response.status < 200 || response.status >= 300) {
          const details = errorMessage(response.body, response.status);
          return yield* new ApiError({
            operation,
            status: response.status,
            ...(details.code === undefined ? {} : { code: details.code }),
            message: details.message,
            ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
          });
        }

        return response.body;
      });

    return {
      request,

      boards: (input) =>
        request("list forum boards", {
          ...input,
          method: "GET",
          path: `${API_VERSION_PATH}/forum`,
        }),

      topics: (input) =>
        request("list forum topics", {
          ...input,
          method: "GET",
          path: `${API_VERSION_PATH}/forum/topics?forum=${encodeURIComponent(input.board)}${
            input.page === undefined ? "" : `&page=${input.page}`
          }`,
        }),

      search: (input) =>
        request("search forum topics", {
          ...input,
          method: "GET",
          path: `${API_VERSION_PATH}/forum/topics?q=${encodeURIComponent(input.query)}${
            input.board === undefined ? "" : `&forum=${encodeURIComponent(input.board)}`
          }${input.page === undefined ? "" : `&page=${input.page}`}`,
        }),

      topic: (input) =>
        request("read a forum topic", {
          ...input,
          method: "GET",
          path: `${API_VERSION_PATH}/forum/topics/${encodeURIComponent(input.id)}${
            input.page === undefined ? "" : `?page=${input.page}`
          }`,
        }),

      createTopic: (input) =>
        request("create a forum topic", {
          ...input,
          method: "POST",
          path: `${API_VERSION_PATH}/forum/topics`,
          body: { forum: input.board, title: input.title, body_text: input.bodyText },
        }),

      reply: (input) =>
        request("reply to a forum topic", {
          ...input,
          method: "POST",
          path: `${API_VERSION_PATH}/forum/topics/${encodeURIComponent(input.topicId)}/posts`,
          body: { body_text: input.bodyText },
        }),

      claim: (input) =>
        request("claim a legacy forum identity", {
          ...input,
          method: "POST",
          path: `${API_VERSION_PATH}/forum/claims`,
          body: { actor_ref: input.actorRef },
        }),

      claims: (input) =>
        request("list identity claims", {
          ...input,
          method: "GET",
          path: `${API_VERSION_PATH}/forum/claims`,
        }),
    };
  }),
);
