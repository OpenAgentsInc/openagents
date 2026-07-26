import { Effect, Option, Redacted, Schema } from "effect";
import type { Event as NostrEvent } from "nostr-effect/pure";
import {
  ForgeMergeGateError,
  ForgeMergeGateInput,
  ForgeGitHubMirrorIntent,
  prepareForgeMergeOutcome,
  type ForgeMergeReceiptStore,
} from "@openagentsinc/forge-protocol";

import { ForgeGitAdmission } from "./admission.js";
import { ForgeGitAuth } from "./auth.js";
import { projectForgeCollaboration } from "./collaboration.js";
import { ForgeGitConfiguration } from "./config.js";
import {
  ForgeGitAuthError,
  ForgeGitAdmissionError,
  type ForgeGitOperation,
  ForgeGitRepositoryError,
  ForgeGitRoute,
  ForgeGitRouteError,
  type ForgeGitScope,
  ForgeGitWebReadError,
} from "./model.js";
import { ForgeGitProjection } from "./projection.js";
import { ForgeGitProjector } from "./projector.js";
import { ForgeGitRepository } from "./repository.js";
import { ForgeGitHubMirrorRunner } from "./github-mirror-runner.js";
import { ForgeWebRead } from "./web-read.js";
import { ForgeWebReadRequest } from "./web-read-model.js";
import { ForgeWebReadPolicy } from "./web-read-policy.js";

const noStoreHeaders = {
  "cache-control": "no-store",
};
const isForgeMergeGateInput = Schema.is(ForgeMergeGateInput);

const maximumTextBytes = 512_000;
const maximumImageBytes = 2_000_000;
const maximumDiffBytes = 1_000_000;

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

const boundedByteLimit = (
  value: string | null,
  defaultValue: number,
  maximumValue: number,
): number | undefined => {
  if (value === null) return defaultValue;
  if (!/^[1-9][0-9]*$/u.test(value)) return undefined;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? Math.min(parsed, maximumValue) : undefined;
};

export const matchForgeWebReadRoute = (request: Request): ForgeWebReadRequest | undefined => {
  const url = new URL(request.url);
  const parts = url.pathname.split("/").filter((part) => part !== "");
  if (
    parts.length !== 6 ||
    parts[0] !== "internal" ||
    parts[1] !== "v1" ||
    parts[2] !== "repositories" ||
    parts[5] !== "web-read"
  ) {
    return undefined;
  }
  const owner = parts[3] === undefined ? undefined : decodeSegment(parts[3]);
  const repo = parts[4] === undefined ? undefined : decodeSegment(parts[4]);
  const view = url.searchParams.get("view");
  const maxTextBytes = boundedByteLimit(
    url.searchParams.get("max_text_bytes"),
    maximumTextBytes,
    maximumTextBytes,
  );
  const maxImageBytes = boundedByteLimit(
    url.searchParams.get("max_image_bytes"),
    maximumImageBytes,
    maximumImageBytes,
  );
  const maxDiffBytes = boundedByteLimit(
    url.searchParams.get("max_diff_bytes"),
    maximumDiffBytes,
    maximumDiffBytes,
  );
  if (
    owner === undefined ||
    repo === undefined ||
    (view !== "code" && view !== "commits" && view !== "commit" && view !== "diff") ||
    maxTextBytes === undefined ||
    maxImageBytes === undefined ||
    maxDiffBytes === undefined
  ) {
    return undefined;
  }
  const optional = (name: string): string | undefined => {
    const value = url.searchParams.get(name);
    return value === null || value === "" ? undefined : value;
  };
  const base = optional("base");
  const commit = optional("commit");
  const ref = optional("ref");
  const encodedPath = url.searchParams.get("path");
  let path: string | undefined;
  try {
    path =
      encodedPath === null
        ? undefined
        : encodedPath
            .split("/")
            .map((segment) => decodeURIComponent(segment))
            .join("/");
  } catch {
    return undefined;
  }
  return ForgeWebReadRequest.make({
    ...(base === undefined ? {} : { base }),
    ...(commit === undefined ? {} : { commit }),
    maxDiffBytes,
    maxImageBytes,
    maxTextBytes,
    owner,
    ...(path === undefined ? {} : { path }),
    ...(ref === undefined ? {} : { ref }),
    repo,
    view,
  });
};

type ForgeWebReadAssetRoute = Readonly<{
  commitId: string;
  maxImageBytes: number;
  objectId: string;
  owner: string;
  path: string;
  repo: string;
}>;

export const matchForgeWebReadAssetRoute = (
  request: Request,
): ForgeWebReadAssetRoute | undefined => {
  const url = new URL(request.url);
  const parts = url.pathname.split("/").filter((part) => part !== "");
  if (
    parts.length < 7 ||
    parts[0] !== "internal" ||
    parts[1] !== "v1" ||
    parts[2] !== "repositories" ||
    parts[5] !== "web-read-asset"
  ) {
    return undefined;
  }
  const owner = parts[3] === undefined ? undefined : decodeSegment(parts[3]);
  const repo = parts[4] === undefined ? undefined : decodeSegment(parts[4]);
  const pathParts = parts.slice(6).map((part) => {
    try {
      return decodeURIComponent(part);
    } catch {
      return undefined;
    }
  });
  const objectId = url.searchParams.get("object");
  const commitId = url.searchParams.get("commit");
  const maxImageBytes = boundedByteLimit(
    url.searchParams.get("max_image_bytes"),
    maximumImageBytes,
    maximumImageBytes,
  );
  if (
    owner === undefined ||
    repo === undefined ||
    objectId === null ||
    objectId === "" ||
    commitId === null ||
    commitId === "" ||
    maxImageBytes === undefined ||
    pathParts.some((part) => part === undefined)
  ) {
    return undefined;
  }
  return {
    commitId,
    maxImageBytes,
    objectId,
    owner,
    path: pathParts.join("/"),
    repo,
  };
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
    error instanceof ForgeGitRepositoryError ||
    error instanceof ForgeGitWebReadError;
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

const recoverRouteError = (error: unknown): Effect.Effect<Response> => {
  const response = errorResponse(error);
  const code =
    error instanceof ForgeGitRouteError ||
    error instanceof ForgeGitAuthError ||
    error instanceof ForgeGitAdmissionError ||
    error instanceof ForgeGitRepositoryError ||
    error instanceof ForgeGitWebReadError
      ? error.code
      : "forge_git_internal_error";
  return Effect.logWarning("Forge Git request failed.", {
    code,
    operation:
      error instanceof ForgeGitRepositoryError || error instanceof ForgeGitWebReadError
        ? error.operation
        : "route",
    status: response.status,
  }).pipe(Effect.as(response));
};

export const forgeWebReadHandler = Effect.fn("ForgeWebReadRoutes.handle")(function* (
  request: Request,
) {
  const assetRoute = matchForgeWebReadAssetRoute(request);
  if (assetRoute !== undefined) {
    if (request.method !== "GET") return yield* methodError();
    const policy = yield* ForgeWebReadPolicy;
    yield* policy.authorize({
      authorization: request.headers.get("authorization"),
      cookie: request.headers.get("cookie"),
      owner: assetRoute.owner,
      repo: assetRoute.repo,
    });
    const reader = yield* ForgeWebRead;
    const asset = yield* reader.readAsset(assetRoute);
    return new Response(responseBody(asset.bytes), {
      headers: {
        ...noStoreHeaders,
        "content-security-policy": "default-src 'none'; sandbox",
        "content-type": asset.contentType,
        "x-content-type-options": "nosniff",
      },
      status: 200,
    });
  }
  const webReadRequest = matchForgeWebReadRoute(request);
  if (webReadRequest === undefined) {
    return Response.json({ error: "not_found" }, { headers: noStoreHeaders, status: 404 });
  }
  if (request.method !== "GET") return yield* methodError();
  const policy = yield* ForgeWebReadPolicy;
  const decision = yield* policy.authorize({
    authorization: request.headers.get("authorization"),
    cookie: request.headers.get("cookie"),
    owner: webReadRequest.owner,
    repo: webReadRequest.repo,
  });
  const reader = yield* ForgeWebRead;
  const projection = yield* reader.read({
    policy: decision,
    request: webReadRequest,
  });
  return Response.json(projection, {
    headers: noStoreHeaders,
    status: 200,
  });
});

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
  const configuration = yield* ForgeGitConfiguration;
  const publicRead =
    route.operation === "git-upload-pack" &&
    configuration.publicReadRepositories.has(`${route.tenantRef}/${route.repositoryRef}`);
  const session = publicRead
    ? {
        actorBindingRef: "public_read",
        authenticatedAt: new Date().toISOString(),
        refRestrictions: [],
        repositoryRef: route.repositoryRef,
        subjectRef: "public_read",
        tenantRef: route.tenantRef,
        tokenRef: "public_read",
      }
    : yield* (yield* ForgeGitAuth).authenticate({
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
      const signedRefPolicies = yield* admission
        .signedRefPolicies({
          repositoryRef: route.repositoryRef,
          tenantRef: route.tenantRef,
        })
        .pipe(
          Effect.tapError((error) =>
            Effect.logWarning("Forge Git receive stage failed.", {
              code: error.code,
              operation: "signed-ref-policies",
            }),
          ),
        );
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
  ).pipe(
    Effect.tapError((error) =>
      Effect.logWarning("Forge Git receive stage failed.", {
        code:
          error instanceof ForgeGitAdmissionError || error instanceof ForgeGitRepositoryError
            ? error.code
            : "forge_git_internal_error",
        operation: "receive-lease",
      }),
    ),
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
    yield* admission
      .recordCommittedReceive({
        repositoryRef: route.repositoryRef,
        stateEventIds: signedRefPolicies
          .filter((policy) => changedRefNames.has(policy.refName))
          .map((policy) => policy.eventId),
        tenantRef: route.tenantRef,
      })
      .pipe(
        Effect.tapError((error) =>
          Effect.logWarning("Forge Git receive stage failed.", {
            code: error.code,
            operation: "record-committed-receive",
          }),
        ),
      );
    yield* admission.recordUnclaimedNostrRefs({
      refNames: result.refsAfter
        .filter((ref) => ref.refName.startsWith("refs/nostr/"))
        .filter(
          (ref) =>
            result.refsBefore.find((before) => before.refName === ref.refName)?.objectId !==
            ref.objectId,
        )
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
const internalMirrorObservePath = "/internal/v1/github-mirror/observe";
const internalMirrorProjectPath = "/internal/v1/github-mirror/project";
const internalMirrorSourcePath = "/internal/v1/github-mirror/source";

const internalMirrorHandler = Effect.fn("ForgeGitRoutes.internalMirror")(function* (
  request: Request,
) {
  const path = new URL(request.url).pathname;
  if (
    path !== internalMirrorObservePath &&
    path !== internalMirrorProjectPath &&
    path !== internalMirrorSourcePath
  ) {
    return undefined;
  }
  const configuration = yield* ForgeGitConfiguration;
  const expected = configuration.internalServiceAuthToken;
  const presented = request.headers.get("authorization")?.replace(/^Bearer\s+/iu, "");
  if (expected === undefined || presented === undefined || presented !== Redacted.value(expected)) {
    return Response.json(
      { error: "forge_github_mirror_service_unauthorized" },
      { headers: noStoreHeaders, status: 401 },
    );
  }
  if (path === internalMirrorSourcePath) {
    if (request.method !== "GET") {
      return Response.json({ error: "method_not_allowed" }, { headers: noStoreHeaders, status: 405 });
    }
    const url = new URL(request.url);
    const tenantRef = url.searchParams.get("tenantRef");
    const repositoryRef = url.searchParams.get("repositoryRef");
    const sourceRef = url.searchParams.get("sourceRef");
    if (tenantRef === null || repositoryRef === null || sourceRef === null || !sourceRef.startsWith("refs/")) {
      return Response.json({ error: "forge_github_mirror_source_request_invalid" }, { headers: noStoreHeaders, status: 400 });
    }
    const repository = yield* ForgeGitRepository;
    const ref = (yield* repository.listRefs({ repositoryRef, tenantRef })).find(
      (item) => item.refName === sourceRef,
    );
    return ref === undefined
      ? Response.json({ error: "not_found" }, { headers: noStoreHeaders, status: 404 })
      : Response.json(
          { objectId: ref.objectId.toLowerCase(), repositoryRef, sourceRef, tenantRef },
          { headers: noStoreHeaders, status: 200 },
        );
  }
  if (request.method !== "POST") {
    return Response.json({ error: "method_not_allowed" }, { headers: noStoreHeaders, status: 405 });
  }
  const raw = yield* Effect.tryPromise({
    try: () => request.json() as Promise<unknown>,
    catch: () => new ForgeGitRouteError({ code: "forge_github_mirror_body_invalid", status: 400 }),
  });
  const intent = yield* Schema.decodeUnknownEffect(ForgeGitHubMirrorIntent)(raw).pipe(
    Effect.mapError(() => new ForgeGitRouteError({ code: "forge_github_mirror_body_invalid", status: 400 })),
  );
  const runner = yield* ForgeGitHubMirrorRunner;
  const observed = yield* (path === internalMirrorObservePath
    ? runner.observe(intent)
    : runner.project(intent)).pipe(
      // The runner reason is a closed, credential-free operational code. Keep
      // it intact so the caller can distinguish a refused projection from a
      // transient transport failure without exposing Git stderr or secrets.
      Effect.mapError((error) =>
        new ForgeGitRouteError({
          code: error.reason,
          status: error.retryable ? 503 : 409,
        }),
      ),
    );
  return Response.json(observed, { headers: noStoreHeaders, status: 200 });
});
const internalMergeReceiptsPath = "/internal/forge/merge-receipts";
const collaborationReadPrefix = "/internal/v1/repositories/";

const matchCollaborationRead = (
  request: Request,
):
  | Readonly<{
      request: import("@openagentsinc/forge-protocol").ForgeCollaborationRequest;
      repositoryRef: string;
      tenantRef: string;
    }>
  | undefined => {
  const parts = new URL(request.url).pathname.split("/").filter((part) => part !== "");
  if (
    parts.length < 7 ||
    parts[0] !== "internal" ||
    parts[1] !== "v1" ||
    parts[2] !== "repositories" ||
    parts[5] !== "collaboration"
  ) {
    return undefined;
  }
  const tenantRef = parts[3] === undefined ? undefined : decodeSegment(parts[3]);
  const repositoryRef = parts[4] === undefined ? undefined : decodeSegment(parts[4]);
  if (tenantRef === undefined || repositoryRef === undefined) return undefined;
  if (parts[6] === "attention" && parts.length === 7) {
    return {
      repositoryRef,
      request: { owner: tenantRef, repo: repositoryRef, view: "attention" },
      tenantRef,
    };
  }
  const recordRef = parts[7] === undefined ? undefined : decodeSegment(parts[7]);
  if (recordRef === undefined || parts.length !== 8) return undefined;
  if (parts[6] === "changes") {
    return {
      repositoryRef,
      request: { changeRef: recordRef, owner: tenantRef, repo: repositoryRef, view: "change" },
      tenantRef,
    };
  }
  if (parts[6] === "work") {
    return {
      repositoryRef,
      request: { owner: tenantRef, repo: repositoryRef, view: "work", workRef: recordRef },
      tenantRef,
    };
  }
  return undefined;
};

/** The web app is a private reader. Membership remains in the policy service;
 * the service bearer only authorizes this internal hop. */
const internalCollaborationReadHandler = Effect.fn("ForgeGitRoutes.internalCollaborationRead")(
  function* (request: Request) {
    if (!new URL(request.url).pathname.startsWith(collaborationReadPrefix)) return undefined;
    const route = matchCollaborationRead(request);
    if (route === undefined) return undefined;
    if (request.method !== "GET") {
      return Response.json(
        { error: "method_not_allowed" },
        { headers: noStoreHeaders, status: 405 },
      );
    }
    const configuration = yield* ForgeGitConfiguration;
    const presented = request.headers.get("authorization")?.replace(/^Bearer\s+/iu, "");
    if (
      presented === undefined ||
      presented !== Redacted.value(configuration.policyAuthorityToken)
    ) {
      return Response.json(
        { error: "forge_collaboration_read_unauthorized" },
        { headers: noStoreHeaders, status: 401 },
      );
    }
    const policyUrl = new URL(configuration.policyAuthorityUrl);
    policyUrl.pathname = "/internal/forge/collaboration-read-authorize";
    policyUrl.search = "";
    const policy = yield* Effect.tryPromise({
      try: () =>
        fetch(policyUrl, {
          body: JSON.stringify({ repositoryRef: route.repositoryRef, tenantRef: route.tenantRef }),
          headers: {
            authorization: `Bearer ${Redacted.value(configuration.policyAuthorityToken)}`,
            "content-type": "application/json",
            ...(request.headers.get("cookie") === null
              ? {}
              : { cookie: request.headers.get("cookie") ?? "" }),
          },
          method: "POST",
        }),
      catch: () =>
        new ForgeGitRouteError({ code: "forge_collaboration_policy_unavailable", status: 503 }),
    });
    if (!policy.ok) {
      return Response.json(
        {
          error:
            policy.status === 401 || policy.status === 403
              ? "forge_collaboration_membership_required"
              : "forge_collaboration_policy_unavailable",
        },
        {
          headers: noStoreHeaders,
          status: policy.status === 401 || policy.status === 403 ? policy.status : 503,
        },
      );
    }
    const admission = yield* ForgeGitAdmission;
    yield* admission.requireRepository({
      repositoryRef: route.repositoryRef,
      tenantRef: route.tenantRef,
    });
    const events = yield* admission.listProjectedEvents({
      repositoryRef: route.repositoryRef,
      tenantRef: route.tenantRef,
    });
    const projection = projectForgeCollaboration(route.request, events);
    if (
      (route.request.view === "change" && projection.change === null) ||
      (route.request.view === "work" && projection.work === null)
    ) {
      return Response.json(
        { error: "forge_collaboration_record_not_found" },
        { headers: noStoreHeaders, status: 404 },
      );
    }
    return Response.json(projection, { headers: noStoreHeaders });
  },
);

const resolveActiveNostrBinding = async (
  configuration: {
    readonly policyAuthorityToken: import("effect").Redacted.Redacted<string>;
    readonly policyAuthorityUrl: string;
  },
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
    return body.tenantRef === tenantRef && typeof body.bindingRef === "string"
      ? body.bindingRef
      : undefined;
  } catch {
    return undefined;
  }
};

const internalAdmissionHandler = Effect.fn("ForgeGitRoutes.internalAdmission")(function* (
  request: Request,
) {
  const path = new URL(request.url).pathname;
  if (
    path !== internalAdmissionPath &&
    path !== internalMergeReceiptsPath &&
    !path.startsWith(`${internalMergeReceiptsPath}/`)
  ) {
    return undefined;
  }
  const configuration = yield* ForgeGitConfiguration;
  const presented = request.headers.get("authorization")?.replace(/^Bearer\s+/iu, "");
  if (presented === undefined || presented !== Redacted.value(configuration.policyAuthorityToken)) {
    return Response.json(
      { error: "forge_git_admission_unauthorized" },
      { headers: noStoreHeaders, status: 401 },
    );
  }
  const admission = yield* ForgeGitAdmission;
  if (path.startsWith(`${internalMergeReceiptsPath}/`)) {
    if (request.method !== "GET") {
      return Response.json(
        { error: "method_not_allowed" },
        { headers: noStoreHeaders, status: 405 },
      );
    }
    const receiptRef = decodeURIComponent(path.slice(internalMergeReceiptsPath.length + 1));
    const tenantRef = new URL(request.url).searchParams.get("tenantRef");
    const repositoryRef = new URL(request.url).searchParams.get("repositoryRef");
    if (tenantRef === null || repositoryRef === null || receiptRef.length === 0) {
      return Response.json(
        { error: "forge_git_merge_receipt_request_invalid" },
        { headers: noStoreHeaders, status: 400 },
      );
    }
    const receipt = yield* admission.readMergeReceipt({ receiptRef, repositoryRef, tenantRef });
    return receipt === undefined
      ? Response.json({ error: "not_found" }, { headers: noStoreHeaders, status: 404 })
      : Response.json(receipt, { headers: noStoreHeaders, status: 200 });
  }
  if (path === internalMergeReceiptsPath) {
    if (request.method !== "POST") {
      return Response.json(
        { error: "method_not_allowed" },
        { headers: noStoreHeaders, status: 405 },
      );
    }
    const body = yield* Effect.tryPromise({
      try: () => request.json() as Promise<unknown>,
      catch: () =>
        new ForgeGitRouteError({ code: "forge_git_merge_receipt_body_invalid", status: 400 }),
    });
    if (typeof body !== "object" || body === null) {
      return Response.json(
        { error: "forge_git_merge_receipt_body_invalid" },
        { headers: noStoreHeaders, status: 400 },
      );
    }
    const record = body as Record<string, unknown>;
    if (typeof record.receiptRef !== "string" || record.receiptRef.length === 0) {
      return Response.json(
        { error: "forge_git_merge_receipt_body_invalid" },
        { headers: noStoreHeaders, status: 400 },
      );
    }
    if (!isForgeMergeGateInput(record.gateInput)) {
      return Response.json(
        { error: "forge_git_merge_receipt_body_invalid" },
        { headers: noStoreHeaders, status: 400 },
      );
    }
    const gateInput = record.gateInput;
    const receipts: ForgeMergeReceiptStore = {
      prepare: (receipt) =>
        admission.prepareMergeReceipt(receipt).pipe(
          Effect.mapError(
            () =>
              new ForgeMergeGateError({
                blockers: ["receipt.store.unavailable"],
                code: "forge_merge_receipt_conflict",
              }),
          ),
        ),
      finalize: () =>
        Effect.die("Forge Git prepares a receipt before its external NIP-46 signer runs."),
    };
    return yield* prepareForgeMergeOutcome(gateInput, record.receiptRef, receipts).pipe(
      Effect.match({
        onFailure: (error) =>
          Response.json(
            { blockers: error.blockers, error: error.code },
            { headers: noStoreHeaders, status: 409 },
          ),
        onSuccess: (receipt) => Response.json(receipt, { headers: noStoreHeaders, status: 201 }),
      }),
    );
  }
  if (request.method !== "POST") {
    return Response.json({ error: "method_not_allowed" }, { headers: noStoreHeaders, status: 405 });
  }
  const body = yield* Effect.tryPromise({
    try: () => request.json() as Promise<unknown>,
    catch: () => new ForgeGitRouteError({ code: "forge_git_admission_body_invalid", status: 400 }),
  });
  if (typeof body !== "object" || body === null) {
    return Response.json(
      { error: "forge_git_admission_body_invalid" },
      { headers: noStoreHeaders, status: 400 },
    );
  }
  const record = body as Record<string, unknown>;
  if (
    typeof record.repositoryRef !== "string" ||
    typeof record.tenantRef !== "string" ||
    typeof record.event !== "object" ||
    record.event === null ||
    typeof (record.event as { pubkey?: unknown }).pubkey !== "string"
  ) {
    return Response.json(
      { error: "forge_git_admission_body_invalid" },
      { headers: noStoreHeaders, status: 400 },
    );
  }
  const tenantRef = record.tenantRef as string;
  const repositoryRef = record.repositoryRef as string;
  const event = record.event as NostrEvent;
  const actorBindingRef = yield* Effect.tryPromise({
    try: () => resolveActiveNostrBinding(configuration, tenantRef, event.pubkey),
    catch: () => new ForgeGitRouteError({ code: "forge_git_membership_unavailable", status: 503 }),
  });
  if (actorBindingRef === undefined) {
    return Response.json(
      { error: "forge_git_membership_forbidden" },
      { headers: noStoreHeaders, status: 403 },
    );
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
  | ForgeGitHubMirrorRunner
  | ForgeGitProjection
  | ForgeGitProjector
  | ForgeGitRepository
  | ForgeWebRead
  | ForgeWebReadPolicy
> => {
  if (
    matchForgeWebReadRoute(request) !== undefined ||
    matchForgeWebReadAssetRoute(request) !== undefined
  ) {
    return forgeWebReadHandler(request).pipe(
      Effect.catch(recoverRouteError),
    );
  }
  return internalCollaborationReadHandler(request).pipe(
    Effect.flatMap((collaboration) =>
      collaboration === undefined
        ? internalMirrorHandler(request).pipe(
            Effect.flatMap((mirror) =>
              mirror === undefined ? internalAdmissionHandler(request) : Effect.succeed(mirror),
            ),
          )
        : Effect.succeed(collaboration),
    ),
    Effect.flatMap((internal) =>
      internal === undefined ? forgeGitHandler(request) : Effect.succeed(internal),
    ),
    Effect.catch(recoverRouteError),
  );
};
