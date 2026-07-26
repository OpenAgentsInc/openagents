import { Effect, Option, Redacted, Schema } from "effect";
import type { Event as NostrEvent } from "nostr-effect/pure";

import { ForgeGitAdmission } from "./admission.js";
import { ForgeGitAuth } from "./auth.js";
import { ForgeGitConfiguration } from "./config.js";
import {
  ForgeGitAuthError,
  ForgeGitAdmissionError,
  type ForgeGitOperation,
  ForgeGitRepositoryError,
  ForgeGitRoute,
  ForgeGitRouteError,
  type ForgeGitScope,
} from "./model.js";
import { ForgeGitProjection } from "./projection.js";
import { ForgeGitProjector } from "./projector.js";
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
    error instanceof ForgeGitAdmissionError ||
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
  const admission = yield* ForgeGitAdmission;

  // Auth answers who may use transport. Admission answers whether this
  // repository exists at all. Keep both checks before invoking stock Git.
  yield* admission.requireRepository({
    repositoryRef: route.repositoryRef,
    tenantRef: route.tenantRef,
  });

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

  const projection = yield* ForgeGitProjection;
  const admittedReceive = yield* admission.withReceiveLease(
    `${route.tenantRef}/${route.repositoryRef}`,
    Effect.gen(function* () {
      const signedRefPolicies = yield* admission.signedRefPolicies({
        repositoryRef: route.repositoryRef,
        tenantRef: route.tenantRef,
      });
      const result = yield* repository.receivePack({
        body: request.body,
        ...(gitProtocol === undefined ? {} : { gitProtocol }),
        repositoryRef: route.repositoryRef,
        session,
        signedRefPolicies,
        tenantRef: route.tenantRef,
      });
      return { result, signedRefPolicies };
    }),
  );
  const { result, signedRefPolicies } = admittedReceive;
  if (result.changed) {
    const changedRefNames = new Set([
      ...result.refsAfter
        .filter(
          (ref) =>
            result.refsBefore.find((before) => before.refName === ref.refName)?.objectId !==
            ref.objectId,
        )
        .map((ref) => ref.refName),
      ...result.refsBefore
        .filter((ref) => !result.refsAfter.some((after) => after.refName === ref.refName))
        .map((ref) => ref.refName),
    ]);
    yield* admission.recordCommittedReceive({
      repositoryRef: route.repositoryRef,
      stateEventIds: signedRefPolicies
        .filter((policy) => changedRefNames.has(policy.refName))
        .map((policy) => policy.eventId),
      tenantRef: route.tenantRef,
    });
    yield* admission.recordUnclaimedNostrRefs({
      refNames: result.refsAfter
        .filter((ref) => ref.refName.startsWith("refs/nostr/"))
        .filter((ref) => result.refsBefore.find((before) => before.refName === ref.refName)?.objectId !== ref.objectId)
        .map((ref) => ref.refName),
      repositoryRef: route.repositoryRef,
      tenantRef: route.tenantRef,
    });
  }
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

const internalAdmissionPath = "/internal/forge/admission/events";

const resolveActiveNostrBinding = async (
  configuration: { readonly policyAuthorityToken: import("effect").Redacted.Redacted<string>; readonly policyAuthorityUrl: string },
  tenantRef: string,
  pubkey: string,
): Promise<string | undefined> => {
  const url = new URL(configuration.policyAuthorityUrl);
  url.pathname = "/internal/forge/relay-admit";
  url.search = "";
  try {
    const response = await fetch(url, {
      body: JSON.stringify({ pubkey, tenantRef }),
      headers: {
        authorization: `Bearer ${Redacted.value(configuration.policyAuthorityToken)}`,
        "content-type": "application/json",
      },
      method: "POST",
    });
    if (!response.ok) return undefined;
    const body = (await response.json()) as { bindingRef?: unknown; tenantRef?: unknown };
    return body.tenantRef === tenantRef && typeof body.bindingRef === "string" ? body.bindingRef : undefined;
  } catch {
    return undefined;
  }
};

const internalAdmissionHandler = Effect.fn("ForgeGitRoutes.internalAdmission")(function* (
  request: Request,
) {
  if (new URL(request.url).pathname !== internalAdmissionPath) return undefined;
  if (request.method !== "POST") {
    return Response.json({ error: "method_not_allowed" }, { headers: noStoreHeaders, status: 405 });
  }
  const configuration = yield* ForgeGitConfiguration;
  const presented = request.headers.get("authorization")?.replace(/^Bearer\s+/iu, "");
  if (presented === undefined || presented !== Redacted.value(configuration.policyAuthorityToken)) {
    return Response.json({ error: "forge_git_admission_unauthorized" }, { headers: noStoreHeaders, status: 401 });
  }
  const body = yield* Effect.tryPromise({
    try: () => request.json() as Promise<unknown>,
    catch: () => new ForgeGitRouteError({ code: "forge_git_admission_body_invalid", status: 400 }),
  });
  if (typeof body !== "object" || body === null) {
    return Response.json({ error: "forge_git_admission_body_invalid" }, { headers: noStoreHeaders, status: 400 });
  }
  const record = body as Record<string, unknown>;
  if (
    typeof record.repositoryRef !== "string" ||
    typeof record.tenantRef !== "string" ||
    typeof record.event !== "object" ||
    record.event === null ||
    typeof (record.event as { pubkey?: unknown }).pubkey !== "string"
  ) {
    return Response.json({ error: "forge_git_admission_body_invalid" }, { headers: noStoreHeaders, status: 400 });
  }
  const tenantRef = record.tenantRef as string;
  const repositoryRef = record.repositoryRef as string;
  const event = record.event as NostrEvent;
  const actorBindingRef = yield* Effect.tryPromise({
    try: () => resolveActiveNostrBinding(
      configuration,
      tenantRef,
      event.pubkey,
    ),
    catch: () => new ForgeGitRouteError({ code: "forge_git_membership_unavailable", status: 503 }),
  });
  if (actorBindingRef === undefined) {
    return Response.json({ error: "forge_git_membership_forbidden" }, { headers: noStoreHeaders, status: 403 });
  }
  const projector = yield* ForgeGitProjector;
  const disposition = yield* projector.project({
    actorBindingRef,
    event,
    repositoryRef,
    tenantRef,
  });
  return Response.json({ disposition }, { headers: noStoreHeaders, status: 202 });
});

export const routeRequest = (
  request: Request,
): Effect.Effect<
  Response,
  never,
  | ForgeGitAdmission
  | ForgeGitAuth
  | ForgeGitConfiguration
  | ForgeGitProjection
  | ForgeGitProjector
  | ForgeGitRepository
> =>
  internalAdmissionHandler(request).pipe(
    Effect.flatMap((internal) => internal === undefined ? forgeGitHandler(request) : Effect.succeed(internal)),
    Effect.catch((error) => Effect.succeed(errorResponse(error))),
  );
