import "@tanstack/react-start/server-only";

import {
  ForgeCollaborationFailure,
  ForgeCollaborationProjection,
  ForgeCollaborationReader,
  ForgeCollaborationResult,
  ForgeCollaborationTransportError,
  type ForgeCollaborationRequest,
  type ForgeCollaborationResult as ForgeCollaborationResultType,
} from "@/features/forge/collaboration-read";
import { getRequestHeader } from "@tanstack/react-start/server";
import { Config, Effect, Exit, Layer, Schema as S } from "effect";

type Fetch = typeof fetch;

export const forgeCollaborationReadUrl = (
  baseUrl: string,
  request: ForgeCollaborationRequest,
): string => {
  const suffix =
    request.view === "attention"
      ? "/attention"
      : request.view === "change"
        ? `/changes/${encodeURIComponent(request.changeRef ?? "")}`
        : `/work/${encodeURIComponent(request.workRef ?? "")}`;
  return new URL(
    `/internal/v1/repositories/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}/collaboration${suffix}`,
    baseUrl,
  ).href;
};

const failed = (
  tag: "not_found" | "authentication_required" | "unavailable" | "malformed_response",
  detail: string,
  retryable = false,
): ForgeCollaborationResultType =>
  ForgeCollaborationResult.cases.failed.make({
    failure:
      tag === "unavailable"
        ? ForgeCollaborationFailure.cases.unavailable.make({ detail, retryable })
        : tag === "not_found"
          ? ForgeCollaborationFailure.cases.not_found.make({ detail })
          : tag === "authentication_required"
            ? ForgeCollaborationFailure.cases.authentication_required.make({ detail })
            : ForgeCollaborationFailure.cases.malformed_response.make({ detail }),
  });

export const makeForgeCollaborationReaderLayer = (baseUrl: string, fetchFn: Fetch = fetch) =>
  Layer.succeed(
    ForgeCollaborationReader,
    ForgeCollaborationReader.of({
      read: Effect.fn("ForgeCollaborationReader.read")(function* (request, authorizationCookie) {
        const response = yield* Effect.tryPromise({
          try: (signal) =>
            fetchFn(forgeCollaborationReadUrl(baseUrl, request), {
              signal,
              cache: "no-store",
              headers: {
                accept: "application/json",
                ...(authorizationCookie === undefined ? {} : { cookie: authorizationCookie }),
              },
            }),
          catch: (cause) =>
            new ForgeCollaborationTransportError({
              operation: "ForgeCollaborationReader.fetch",
              cause,
            }),
        });
        if (response.status === 401 || response.status === 403) {
          return failed("authentication_required", "This Forge collaboration view requires an invitation.");
        }
        if (response.status === 404) return failed("not_found", "The requested Forge record was not found.");
        if (!response.ok) {
          return failed("unavailable", `The owned Forge collaboration service returned HTTP ${response.status}.`, response.status >= 500);
        }
        const payload = yield* Effect.tryPromise({
          try: () => response.json(),
          catch: (cause) => new ForgeCollaborationTransportError({ operation: "ForgeCollaborationReader.responseJson", cause }),
        });
        const projection = yield* S.decodeUnknownEffect(ForgeCollaborationProjection)(payload).pipe(
          Effect.mapError((cause) => new ForgeCollaborationTransportError({ operation: "ForgeCollaborationReader.decode", cause })),
        );
        return ForgeCollaborationResult.cases.loaded.make({ projection });
      }),
    }),
  );

const runRead = (request: ForgeCollaborationRequest, authorizationCookie: string | undefined) =>
  Effect.gen(function* () {
    const reader = yield* ForgeCollaborationReader;
    return yield* reader.read(request, authorizationCookie);
  }).pipe(
    Effect.provide(
      Layer.unwrap(
        Config.string("OPENAGENTS_FORGE_READ_BASE_URL").pipe(
          Effect.map((baseUrl) => makeForgeCollaborationReaderLayer(baseUrl)),
        ),
      ),
    ),
  );

export const readForgeCollaborationFromOwnedService = async (
  request: ForgeCollaborationRequest,
): Promise<ForgeCollaborationResultType> => {
  const exit = await Effect.runPromiseExit(runRead(request, getRequestHeader("cookie")));
  return Exit.isSuccess(exit)
    ? exit.value
    : failed("unavailable", "The owned Forge collaboration service is not configured or did not return a valid projection.", true);
};
