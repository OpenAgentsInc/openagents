/**
 * The request seam the issue and project clients share.
 *
 * Both talk to the same `/api/v3` routes behind the same unified error
 * envelope from issue #82, so the transport call, the accepted-status check,
 * and the failure translation live once rather than twice.
 */

import { Effect } from "effect";

import type { ApiTransportInterface, HttpMethod } from "./api-transport.js";
import { ApiError } from "./errors.js";
import type { AuthenticatedApi } from "./repository-client.js";

export interface TrackerRequestInput extends AuthenticatedApi {
  readonly method: HttpMethod;
  readonly path: string;
  readonly body?: unknown;
  readonly acceptedStatuses: ReadonlyArray<number>;
}

/** Reads a JSON object, or an empty one when the value is not an object. */
export const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

/** Reads a named array of objects out of an envelope, such as `issues`. */
export const asRows = (value: unknown, key: string): ReadonlyArray<Record<string, unknown>> => {
  const list = asRecord(value)[key];
  return Array.isArray(list) ? list.map(asRecord) : [];
};

export const asText = (value: unknown): string | undefined =>
  typeof value === "string" ? value : undefined;

export const asNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

const messageList = (value: unknown): string =>
  Array.isArray(value)
    ? value.map((item) => (typeof item === "string" ? item : JSON.stringify(item))).join(", ")
    : typeof value === "string"
      ? value
      : JSON.stringify(value);

export interface TrackerErrorDetails {
  readonly message: string;
  readonly code?: string;
  readonly requestId?: string;
}

/**
 * Turns the unified error envelope into one sentence.
 *
 * `errors` is a field-to-messages map that is always present, so a rejected
 * write names the field it was rejected on instead of reporting a bare status
 * the caller has to reproduce to understand.
 */
export const trackerErrorDetails = (body: unknown, status: number): TrackerErrorDetails => {
  const envelope = asRecord(body);
  const sentence = asText(envelope["message"]) ?? `The OpenAgents API returned HTTP ${status}.`;
  const fields = Object.entries(asRecord(envelope["errors"])).map(
    ([field, messages]) => `${field}: ${messageList(messages)}`,
  );
  const code = asText(envelope["code"]);
  const requestId = asText(envelope["request_id"]);
  return {
    message: fields.length === 0 ? sentence : `${sentence} (${fields.join("; ")})`,
    ...(code === undefined ? {} : { code }),
    ...(requestId === undefined ? {} : { requestId }),
  };
};

export const makeTrackerRequest = (transport: ApiTransportInterface) =>
  Effect.fn("TrackerRequest.send")(function* (operation: string, input: TrackerRequestInput) {
    const response = yield* transport.request({
      origin: input.origin,
      method: input.method,
      path: input.path,
      token: input.token,
      ...(input.body === undefined ? {} : { body: input.body }),
    });
    if (!input.acceptedStatuses.includes(response.status)) {
      const details = trackerErrorDetails(response.body, response.status);
      return yield* new ApiError({
        operation,
        status: response.status,
        ...(details.code === undefined ? {} : { code: details.code }),
        message: details.message,
        ...(response.requestId === undefined && details.requestId === undefined
          ? {}
          : { requestId: response.requestId ?? details.requestId }),
      });
    }
    return response.body;
  });

export type TrackerRequest = ReturnType<typeof makeTrackerRequest>;

/** The path prefix every repository-scoped tracker route shares. */
export const repositoryPath = (owner: string, repo: string): string =>
  `/api/v3/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;
