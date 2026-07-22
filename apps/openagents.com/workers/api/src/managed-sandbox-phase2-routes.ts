import {
  type ManagedSandboxPhase2Command,
  type ManagedSandboxPhase2Error,
  ManagedSandboxPhase2CommandSchema,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import { parseJsonUnknown } from "./json-boundary";
import type { ManagedSandboxPhase2ExecutionResult } from "./managed-sandbox-phase2-service";

export const MANAGED_SANDBOX_PHASE2_COMMANDS_PATH =
  "/api/managed-sandboxes/phase2/commands" as const;
export const MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION =
  "openagents.managed_sandbox_phase2_api.v1" as const;

const MAX_REQUEST_BYTES = 128 * 1024;

class ManagedSandboxPhase2BodyError extends Error {}

const RequestSchema = S.Struct({
  schemaVersion: S.Literal(MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION),
  command: ManagedSandboxPhase2CommandSchema,
});
const decodeRequest = S.decodeUnknownSync(RequestSchema, {
  onExcessProperty: "error",
});

type AuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ManagedSandboxPhase2RouteDependencies<Bindings> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AuthenticatedOwner | undefined>;
  enabled: (env: Bindings) => boolean;
  execute: (
    env: Bindings,
    command: ManagedSandboxPhase2Command,
  ) => Effect.Effect<ManagedSandboxPhase2ExecutionResult, ManagedSandboxPhase2Error>;
}>;

const json = (
  body: unknown,
  init: ResponseInit = {},
  decorate?: HttpHeadersDecorator,
): Response => {
  const headers = new Headers(init.headers);
  headers.set("content-type", "application/json; charset=utf-8");
  headers.set("cache-control", "no-store");
  decorate?.(headers);
  return new Response(JSON.stringify(body), { ...init, headers });
};

const invalid = (
  requestRef: string,
  message: string,
  retryable = false,
): ManagedSandboxPhase2Error => ({
  _tag: "InvalidRequest",
  requestRef,
  message,
  retryable,
  evidenceRefs: [],
});

const errorStatus = (error: ManagedSandboxPhase2Error): number => {
  switch (error["_tag"]) {
    case "InvalidRequest":
      return error.retryable ? 503 : 400;
    case "CheckpointIncomplete":
      return 404;
    case "CheckpointExpired":
    case "PrivateIngressRevoked":
    case "PrivateIngressExpired":
      return 410;
    case "PrivateIngressUnavailable":
      return 501;
    case "ResumeFailed":
      return error.retryable ? 503 : 409;
    case "IdempotencyConflict":
    case "CheckpointCorrupt":
    case "StaleSource":
    case "DuplicateFork":
      return 409;
  }
};

const errorResponse = (
  error: ManagedSandboxPhase2Error,
  decorate?: HttpHeadersDecorator,
): Response =>
  json(
    {
      schemaVersion: MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
      error,
    },
    { status: errorStatus(error) },
    decorate,
  );

const collectRequestChunks = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  chunks: ReadonlyArray<Uint8Array> = [],
  byteLength = 0,
): Promise<Readonly<{ chunks: ReadonlyArray<Uint8Array>; byteLength: number }>> => {
  const next = await reader.read();
  if (next.done) return { chunks, byteLength };
  const nextByteLength = byteLength + next.value.byteLength;
  if (nextByteLength > MAX_REQUEST_BYTES) {
    await reader.cancel().catch(() => undefined);
    throw new ManagedSandboxPhase2BodyError();
  }
  return collectRequestChunks(reader, [...chunks, next.value], nextByteLength);
};

const readBoundedJson = async (request: Request): Promise<unknown> => {
  const contentLength = request.headers.get("content-length");
  const declaredLength =
    contentLength === null || contentLength.trim() === "" ? undefined : Number(contentLength);
  if (
    declaredLength !== undefined &&
    (!Number.isSafeInteger(declaredLength) ||
      declaredLength < 0 ||
      declaredLength > MAX_REQUEST_BYTES)
  ) {
    throw new ManagedSandboxPhase2BodyError();
  }
  if (request.body === null) throw new ManagedSandboxPhase2BodyError();
  const collected = await collectRequestChunks(request.body.getReader());
  const bytes = new Uint8Array(collected.byteLength);
  collected.chunks.reduce((offset, chunk) => {
    bytes.set(chunk, offset);
    return offset + chunk.byteLength;
  }, 0);
  return parseJsonUnknown(new TextDecoder().decode(bytes));
};

const parseRequest = (request: Request) =>
  Effect.gen(function* () {
    const value = yield* Effect.tryPromise({
      try: () => readBoundedJson(request),
      catch: () =>
        invalid(
          "request.phase2.body",
          "the Phase 2 request body is unavailable or exceeds 128 KiB",
        ),
    });
    return yield* Effect.try({
      try: () => decodeRequest(value),
      catch: () => invalid("request.phase2.invalid", "the Phase 2 request failed validation"),
    });
  });

export const makeManagedSandboxPhase2Routes = <Bindings>(
  deps: ManagedSandboxPhase2RouteDependencies<Bindings>,
) => {
  const commands = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST") {
        return json({ error: "method_not_allowed" }, { status: 405 });
      }
      const owner = yield* Effect.tryPromise({
        try: () => deps.authenticateOwner(request, env, ctx),
        catch: () =>
          invalid(
            "request.phase2.authentication",
            "managed-sandbox Phase 2 authentication is unavailable",
            true,
          ),
      });
      if (owner === undefined) {
        return json({ error: "unauthorized" }, { status: 401 });
      }
      if (!deps.enabled(env)) {
        return errorResponse(
          invalid(
            "request.phase2.activation",
            "managed-sandbox Phase 2 is not admitted on the live target",
            true,
          ),
          owner.decorateResponseHeaders,
        );
      }
      return yield* Effect.gen(function* () {
        const body = yield* parseRequest(request);
        if (body.command.ownerRef !== owner.userId || body.command.tenantRef !== owner.userId) {
          return json(
            { error: "owner_scope_mismatch" },
            { status: 403 },
            owner.decorateResponseHeaders,
          );
        }
        const result = yield* deps.execute(env, body.command);
        return json(
          {
            schemaVersion: MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
            result,
          },
          {},
          owner.decorateResponseHeaders,
        );
      }).pipe(
        Effect.catch((error) =>
          Effect.succeed(errorResponse(error, owner.decorateResponseHeaders)),
        ),
      );
    }).pipe(Effect.catch((error) => Effect.succeed(errorResponse(error))));

  return { commands } as const;
};
