import { Effect, Option, Schema } from "effect";

import { ForgeGitAdmission } from "./admission.js";
import { ForgeGitAuth } from "./auth.js";
import {
  ForgeGitAuthError,
  type ForgeGitOperation,
  ForgeGitRepositoryError,
  ForgeGitRoute,
  ForgeGitRouteError,
  type ForgeGitScope,
} from "./model.js";
import { ForgeGitProjection } from "./projection.js";
import { ForgeGitRepository } from "./repository.js";

const noStoreHeaders = {
  "cache-control": "no-store",
};

const responseBody = (bytes: Uint8Array): ArrayBuffer => {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
};

const decodeSegment = (value: string): string | undefined => {
  try {
    const decoded = decodeURIComponent(value);
    return Schema.is(ForgeGitRoute.fields.tenantRef)(decoded) ? decoded : undefined;
  } catch {
    return undefined;
  }
};

export const matchForgeGitRoute = (request: Request): ForgeGitRoute | undefined => {
  const url = new URL(request.url);
  const parts = url.pathname.split("/").filter((part) => part !== "");
  if (parts[0] !== "git" || parts.length < 3) return undefined;
  const tenantRef = parts[1] === undefined ? undefined : decodeSegment(parts[1]);
  const repositorySegment = parts[2] === undefined ? undefined : decodeSegment(parts[2]);
  if (tenantRef === undefined || repositorySegment === undefined) {
    return undefined;
  }
  const repositoryRef = repositorySegment.endsWith(".git")
    ? repositorySegment.slice(0, -4)
    : repositorySegment;
  if (!Schema.is(ForgeGitRoute.fields.repositoryRef)(repositoryRef)) {
    return undefined;
  }

  if (parts.length === 5 && parts[3] === "info" && parts[4] === "refs") {
    const service = url.searchParams.get("service");
    if (service !== "git-upload-pack" && service !== "git-receive-pack") {
      return undefined;
    }
    return ForgeGitRoute.make({
      kind: "advertisement",
      operation: service,
      repositoryRef,
      tenantRef,
    });
  }
  if (parts.length === 4 && (parts[3] === "git-upload-pack" || parts[3] === "git-receive-pack")) {
    return ForgeGitRoute.make({
      kind: "rpc",
      operation: parts[3],
      repositoryRef,
      tenantRef,
    });
  }
  return undefined;
};

const scopeForOperation = (operation: ForgeGitOperation): ForgeGitScope =>
  operation === "git-upload-pack" ? "git:upload-pack" : "git:receive-pack";

const contentTypeFor = (
  operation: ForgeGitOperation,
  suffix: "advertisement" | "request" | "result",
): string => `application/x-${operation}-${suffix}`;

const validateGitProtocol = (
  request: Request,
): Effect.Effect<string | undefined, ForgeGitRouteError> => {
  const value = request.headers.get("git-protocol")?.trim();
  if (value === undefined || value === "") {
    return Effect.sync((): string | undefined => undefined);
  }
  if (value.length > 128 || !/^[A-Za-z0-9=:. -]+$/.test(value)) {
    return Effect.fail(
      new ForgeGitRouteError({
        code: "forge_git_protocol_header_invalid",
        status: 400,
      }),
    );
  }
  return Effect.succeed(value);
};

const methodError = () =>
  new ForgeGitRouteError({
    code: "forge_git_method_not_allowed",
    status: 405,
  });

const contentTypeError = () =>
  new ForgeGitRouteError({
    code: "forge_git_content_type_unsupported",
    status: 415,
  });

const errorResponse = (error: unknown): Response => {
  const known =
    error instanceof ForgeGitRouteError ||
    error instanceof ForgeGitAuthError ||
    error instanceof ForgeGitRepositoryError;
  const status = known ? error.status : 500;
  const code = known ? error.code : "forge_git_internal_error";
  return Response.json(
    { error: code },
    {
      headers: {
        ...noStoreHeaders,
        ...(status === 401 ? { "www-authenticate": 'Basic realm="OpenAgents Forge Git"' } : {}),
      },
      status,
    },
  );
};

export const forgeGitHandler = Effect.fn("ForgeGitRoutes.handle")(function* (request: Request) {
  const route = matchForgeGitRoute(request);
  if (route === undefined) {
    return Response.json({ error: "not_found" }, { headers: noStoreHeaders, status: 404 });
  }
  if (
    (route.kind === "advertisement" && request.method !== "GET") ||
    (route.kind === "rpc" && request.method !== "POST")
  ) {
    return yield* methodError();
  }
  if (
    route.kind === "rpc" &&
    request.headers.get("content-type")?.split(";", 1)[0]?.trim() !==
      contentTypeFor(route.operation, "request")
  ) {
    return yield* contentTypeError();
  }

  const gitProtocol = yield* validateGitProtocol(request);
  const auth = yield* ForgeGitAuth;
  const session = yield* auth.authenticate({
    authorization: request.headers.get("authorization"),
    nowIso: new Date().toISOString(),
    repositoryRef: route.repositoryRef,
    requiredScope: scopeForOperation(route.operation),
    tenantRef: route.tenantRef,
  });
  const repository = yield* ForgeGitRepository;

  if (route.kind === "advertisement") {
    const body = yield* repository.advertise({
      ...(gitProtocol === undefined ? {} : { gitProtocol }),
      operation: route.operation,
      repositoryRef: route.repositoryRef,
      tenantRef: route.tenantRef,
    });
    return new Response(responseBody(body), {
      headers: {
        ...noStoreHeaders,
        "content-type": contentTypeFor(route.operation, "advertisement"),
      },
      status: 200,
    });
  }

  if (route.operation === "git-upload-pack") {
    const body = yield* repository.uploadPack({
      body: request.body,
      ...(gitProtocol === undefined ? {} : { gitProtocol }),
      repositoryRef: route.repositoryRef,
      tenantRef: route.tenantRef,
    });
    return new Response(body, {
      headers: {
        ...noStoreHeaders,
        "content-type": contentTypeFor(route.operation, "result"),
      },
      status: 200,
    });
  }

  const admission = yield* ForgeGitAdmission;
  const projection = yield* ForgeGitProjection;
  const result = yield* admission.withReceiveLease(
    `${route.tenantRef}/${route.repositoryRef}`,
    repository.receivePack({
      body: request.body,
      ...(gitProtocol === undefined ? {} : { gitProtocol }),
      repositoryRef: route.repositoryRef,
      session,
      tenantRef: route.tenantRef,
    }),
  );
  const projectionReceipt = result.changed
    ? yield* projection
        .projectReceive({
          mirrorReceipt: result.mirrorReceipt,
          refsAfter: result.refsAfter,
          refsBefore: result.refsBefore,
          repositoryRef: route.repositoryRef,
          session,
          tenantRef: route.tenantRef,
        })
        .pipe(
          Effect.map(Option.some),
          Effect.catch((error) =>
            Effect.logWarning("Forge Git ref projection failed.", {
              operation: error.operation,
              repositoryRef: route.repositoryRef,
              tenantRef: route.tenantRef,
            }).pipe(Effect.as(Option.none())),
          ),
        )
    : Option.none();

  return new Response(responseBody(result.body), {
    headers: {
      ...noStoreHeaders,
      "content-type": result.contentType,
      "x-openagents-forge-authority": "bare-repository",
      "x-openagents-forge-mirror": Option.isSome(result.mirrorReceipt) ? "recorded" : "unavailable",
      "x-openagents-forge-projection": Option.isSome(projectionReceipt)
        ? "recorded"
        : "unavailable",
    },
    status: 200,
  });
});

export const routeRequest = (
  request: Request,
): Effect.Effect<
  Response,
  never,
  ForgeGitAdmission | ForgeGitAuth | ForgeGitProjection | ForgeGitRepository
> => forgeGitHandler(request).pipe(Effect.catch((error) => Effect.succeed(errorResponse(error))));
