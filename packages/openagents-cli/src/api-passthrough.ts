import { Effect, Option } from "effect";

import type { HttpMethod } from "./api-transport.js";
import { InputError } from "./errors.js";

/**
 * Every OpenAgents API route lives under this prefix. A passthrough path
 * without a leading slash resolves under it.
 */
export const API_BASE_PATH = "/api/v3/";

/**
 * The methods `openagents api` accepts. The transport supports more, but a
 * passthrough that offers `HEAD`, `OPTIONS`, or `TRACE` promises behavior the
 * API does not have.
 */
export const PASSTHROUGH_METHODS = ["GET", "POST", "PATCH", "PUT", "DELETE"] as const;

export type PassthroughMethod = (typeof PASSTHROUGH_METHODS)[number];

const absoluteUrl = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//u;
const httpUrl = /^https?:\/\//iu;
const headerName = /^[A-Za-z0-9!#$%&'*+.^_`|~-]+$/u;

const leavesOrigin = (candidate: string, origin: string) =>
  new InputError({
    message: `The path ${candidate} leaves the configured API origin ${origin}.`,
  });

/**
 * Turns a caller-supplied path into an origin-relative request path.
 *
 * A path without a leading slash resolves under `/api/v3/`, so
 * `repos/OWNER/REPO/issues` and `/api/v3/repos/OWNER/REPO/issues` name the same
 * route. An absolute path must stay under `/api/`, because this command talks
 * to the API rather than to the website. A complete URL is accepted only when
 * it matches the configured origin.
 */
export const resolveApiPath = Effect.fn("ApiPassthrough.resolveApiPath")(function* (
  origin: string,
  path: string,
) {
  const value = path.trim();
  if (value.length === 0) {
    return yield* new InputError({ message: "The API path cannot be empty." });
  }
  if (value.startsWith("//")) {
    return yield* leavesOrigin(value, origin);
  }

  if (absoluteUrl.test(value)) {
    if (!httpUrl.test(value)) {
      return yield* new InputError({
        message: `The path ${value} must use http or https.`,
      });
    }
    const url = yield* Effect.try({
      try: () => new URL(value),
      catch: () => new InputError({ message: `Invalid API path: ${value}` }),
    });
    if (url.origin !== origin) return yield* leavesOrigin(value, origin);
    return `${url.pathname}${url.search}`;
  }

  const base = value.startsWith("/") ? `${origin}/` : `${origin}${API_BASE_PATH}`;
  const url = yield* Effect.try({
    try: () => new URL(value, base),
    catch: () => new InputError({ message: `Invalid API path: ${value}` }),
  });
  if (url.origin !== origin) return yield* leavesOrigin(value, origin);

  if (value.startsWith("/")) {
    if (!url.pathname.startsWith("/api/")) {
      return yield* new InputError({
        message: `An absolute path must start with /api/. Write ${value.slice(1)} to resolve it under ${API_BASE_PATH}.`,
      });
    }
  } else if (!url.pathname.startsWith(API_BASE_PATH)) {
    return yield* new InputError({
      message: `The path ${value} resolves outside ${API_BASE_PATH}.`,
    });
  }

  return `${url.pathname}${url.search}`;
});

/**
 * Collects repeated `--field key=value` flags into a JSON object. Every value
 * is sent as a JSON string. The command makes no guess about the type a route
 * wants, so `--input` carries numbers, booleans, arrays, and nested objects.
 */
export const parseRequestFields = Effect.fn("ApiPassthrough.parseRequestFields")(function* (
  fields: ReadonlyArray<string>,
) {
  const entries = new Map<string, string>();
  for (const field of fields) {
    const separator = field.indexOf("=");
    if (separator <= 0) {
      return yield* new InputError({
        message: `Use --field key=value. The CLI received ${field}.`,
      });
    }
    const key = field.slice(0, separator);
    if (entries.has(key)) {
      return yield* new InputError({ message: `--field ${key} is set more than once.` });
    }
    entries.set(key, field.slice(separator + 1));
  }
  return Object.fromEntries(entries) as Record<string, string>;
});

/**
 * Collects repeated `--header 'Name: value'` flags. Header names are compared
 * without case. The authorization header comes from the OpenAgents session, so
 * a caller cannot replace it.
 */
export const parseRequestHeaders = Effect.fn("ApiPassthrough.parseRequestHeaders")(function* (
  headers: ReadonlyArray<string>,
) {
  const entries = new Map<string, string>();
  for (const header of headers) {
    const separator = header.indexOf(":");
    if (separator <= 0) {
      return yield* new InputError({
        message: `Use --header 'Name: value'. The CLI received ${header}.`,
      });
    }
    const name = header.slice(0, separator).trim();
    if (!headerName.test(name)) {
      return yield* new InputError({ message: `Invalid header name: ${name}` });
    }
    const normalized = name.toLowerCase();
    if (normalized === "authorization") {
      return yield* new InputError({
        message:
          "The CLI sets the authorization header from your OpenAgents session. Remove --header authorization.",
      });
    }
    entries.set(normalized, header.slice(separator + 1).trim());
  }
  return Object.fromEntries(entries) as Record<string, string>;
});

/**
 * Selects the request method. An explicit `--method` always wins. Without one,
 * a request that carries a body is a `POST` and a request without one is a
 * `GET`, which is how `gh api` behaves.
 */
export const resolveRequestMethod = (
  method: Option.Option<PassthroughMethod>,
  hasBody: boolean,
): HttpMethod => (Option.isSome(method) ? method.value : hasBody ? "POST" : "GET");

export const decodeRequestBody = Effect.fn("ApiPassthrough.decodeRequestBody")(function* (
  text: string,
  source: string,
) {
  const trimmed = text.trim();
  if (trimmed.length === 0) {
    return yield* new InputError({ message: `${source} contained no JSON body.` });
  }
  return yield* Effect.try({
    try: () => JSON.parse(trimmed) as unknown,
    catch: () => new InputError({ message: `${source} did not contain valid JSON.` }),
  });
});

export const successfulStatus = (status: number): boolean => status >= 200 && status < 300;
