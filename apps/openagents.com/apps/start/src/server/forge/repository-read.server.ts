import "@tanstack/react-start/server-only";

import {
  FORGE_MAX_DIFF_BYTES,
  FORGE_MAX_IMAGE_BYTES,
  FORGE_MAX_TEXT_BYTES,
  ForgeReadTransportError,
  ForgeRepositoryProjection,
  ForgeRepositoryReadFailure,
  ForgeRepositoryReader,
  ForgeRepositoryReadResult,
  enforceForgePublicWebRead,
  type ForgeRepositoryReadRequest,
  type ForgeRepositoryReadResult as ForgeRepositoryReadResultType,
} from "@/features/forge/repository-read";
import { getRequestHeader } from "@tanstack/react-start/server";
import { Config, Effect, Exit, Layer, Redacted, Schema as S } from "effect";

type Fetch = typeof fetch;

const safePath = (value: string): string =>
  value
    .split("/")
    .filter((segment) => segment !== "")
    .map(encodeURIComponent)
    .join("/");

export const forgeRepositoryReadUrl = (
  baseUrl: string,
  request: ForgeRepositoryReadRequest,
): string => {
  const url = new URL(
    `/internal/v1/repositories/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}/web-read`,
    baseUrl,
  );
  url.searchParams.set("view", request.view);
  if (request.ref !== undefined) url.searchParams.set("ref", request.ref);
  if (request.path !== undefined && request.path !== "") {
    url.searchParams.set("path", safePath(request.path));
  }
  if (request.commit !== undefined) url.searchParams.set("commit", request.commit);
  if (request.base !== undefined) url.searchParams.set("base", request.base);
  url.searchParams.set("max_text_bytes", String(FORGE_MAX_TEXT_BYTES));
  url.searchParams.set("max_image_bytes", String(FORGE_MAX_IMAGE_BYTES));
  url.searchParams.set("max_diff_bytes", String(FORGE_MAX_DIFF_BYTES));
  return url.href;
};

const failed = (
  tag: "not_found" | "authentication_required" | "unavailable" | "malformed_response",
  detail: string,
  retryable = false,
): ForgeRepositoryReadResultType =>
  ForgeRepositoryReadResult.cases.failed.make({
    failure:
      tag === "unavailable"
        ? ForgeRepositoryReadFailure.cases.unavailable.make({ detail, retryable })
        : tag === "not_found"
          ? ForgeRepositoryReadFailure.cases.not_found.make({ detail })
          : tag === "authentication_required"
            ? ForgeRepositoryReadFailure.cases.authentication_required.make({ detail })
            : ForgeRepositoryReadFailure.cases.malformed_response.make({ detail }),
  });

const ownedProjectionViolation = (
  projection: S.Schema.Type<typeof ForgeRepositoryProjection>,
): string | undefined => {
  const markdownFiles = [
    projection.readme,
    projection.file?._tag === "markdown" ? projection.file : null,
  ];
  const markdownAssets = markdownFiles.flatMap((file) => file?.assets ?? []);
  if (
    projection.repository.authorityMode === "openagents_git_authoritative" &&
    (!projection.repository.canonicalCloneUrl.startsWith("https://openagents.com/git/") ||
      /github\.com/i.test(projection.repository.canonicalCloneUrl))
  ) {
    return "A migrated repository did not use the owned OpenAgents Git clone path.";
  }
  if (
    projection.file?._tag === "image" &&
    (!projection.file.sourceUrl.startsWith("/") ||
      projection.file.sourceUrl.startsWith("//") ||
      /github/i.test(projection.file.sourceUrl))
  ) {
    return "An image preview did not use a same-origin owned Forge path.";
  }
  if (
    markdownAssets.some(
      (asset) =>
        !asset.sourceUrl.startsWith("/") ||
        asset.sourceUrl.startsWith("//") ||
        /github/i.test(asset.sourceUrl),
    )
  ) {
    return "A Markdown image did not use a same-origin owned Forge path.";
  }
  return undefined;
};

export const makeForgeRepositoryReaderLayer = (
  baseUrl: string,
  serviceAuthToken: string,
  fetchFn: Fetch = fetch,
) =>
  Layer.succeed(
    ForgeRepositoryReader,
    ForgeRepositoryReader.of({
      read: Effect.fn("ForgeRepositoryReader.read")(function* (request, authorizationCookie) {
        const response = yield* Effect.tryPromise({
          try: (signal) =>
            fetchFn(forgeRepositoryReadUrl(baseUrl, request), {
              signal,
              cache: "no-store",
              headers: {
                accept: "application/json",
                authorization: `Bearer ${serviceAuthToken}`,
                ...(authorizationCookie === undefined ? {} : { cookie: authorizationCookie }),
              },
            }),
          catch: (cause) =>
            new ForgeReadTransportError({
              operation: "ForgeRepositoryReader.fetch",
              cause,
            }),
        });

        if (response.status === 401 || response.status === 403) {
          return failed("authentication_required", "This repository requires a Forge invitation.");
        }
        if (response.status === 404) {
          return failed("not_found", "The Forge repository or revision was not found.");
        }
        if (!response.ok) {
          return failed(
            "unavailable",
            `The owned Forge read service returned HTTP ${response.status}.`,
            response.status >= 500,
          );
        }

        const payload = yield* Effect.tryPromise({
          try: () => response.json(),
          catch: (cause) =>
            new ForgeReadTransportError({
              operation: "ForgeRepositoryReader.responseJson",
              cause,
            }),
        });
        const decoded = yield* S.decodeUnknownEffect(ForgeRepositoryProjection)(payload).pipe(
          Effect.mapError(
            (cause) =>
              new ForgeReadTransportError({
                operation: "ForgeRepositoryReader.decode",
                cause,
              }),
          ),
        );
        const violation = ownedProjectionViolation(decoded);
        if (violation !== undefined) {
          return failed("malformed_response", violation);
        }
        return ForgeRepositoryReadResult.cases.loaded.make({
          projection: decoded,
        });
      }),
    }),
  );

const runOwnedForgeRead = (
  request: ForgeRepositoryReadRequest,
  authorizationCookie: string | undefined,
) =>
  Effect.gen(function* () {
    const reader = yield* ForgeRepositoryReader;
    return yield* reader.read(request, authorizationCookie);
  }).pipe(
    Effect.provide(
      Layer.unwrap(
        Effect.gen(function* () {
          const baseUrl = yield* Config.string("OPENAGENTS_FORGE_READ_BASE_URL");
          const serviceAuthToken = yield* Config.redacted(
            "OPENAGENTS_FORGE_GIT_SERVICE_AUTH_TOKEN",
          );
          return makeForgeRepositoryReaderLayer(baseUrl, Redacted.value(serviceAuthToken));
        }),
      ),
    ),
  );

export const readForgeRepositoryFromOwnedService = async (
  request: ForgeRepositoryReadRequest,
): Promise<ForgeRepositoryReadResultType> => {
  const authorizationCookie = getRequestHeader("cookie");
  const exit = await Effect.runPromiseExit(runOwnedForgeRead(request, authorizationCookie));
  if (Exit.isFailure(exit)) {
    return failed(
      "unavailable",
      "The owned Forge read service is not configured or did not return a valid projection.",
      true,
    );
  }
  return enforceForgePublicWebRead(exit.value, authorizationCookie !== undefined);
};
