import {
  type ManagedSandboxCommand,
  ManagedSandboxCommandSchema,
  type ManagedSandboxResource,
  SandboxRef,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import { makeManagedSandboxBroker } from "./managed-sandbox-broker";
import type {
  BoxV1NativeStore,
  BoxV1Policy,
  BoxV1Principal,
  BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import { BoxV1FacadeError } from "./managed-sandbox-box-v1-routes";

export const MANAGED_SANDBOX_DESKTOP_ADMISSION_PATH =
  "/api/managed-sandboxes/desktop/admission" as const;
export const MANAGED_SANDBOX_DESKTOP_COMMANDS_PATH =
  "/api/managed-sandboxes/desktop/commands" as const;

type AuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ManagedSandboxDesktopRouteDependencies<Bindings> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AuthenticatedOwner | undefined>;
  enabled: (env: Bindings) => boolean;
  policy: (env: Bindings) => Effect.Effect<BoxV1Policy, BoxV1FacadeError>;
  store: (env: Bindings) => BoxV1NativeStore;
  runtime: (env: Bindings) => Effect.Effect<BoxV1Runtime, BoxV1FacadeError>;
  now?: (() => Date) | undefined;
}>;

const BoundedRef = SandboxRef;
const Timestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);
const AttachmentSchema = S.Struct({
  schemaVersion: S.Literal("openagents.desktop.ide-agent-code.v1"),
  agentAttachmentRef: BoundedRef,
  projectRef: BoundedRef,
  rootRef: BoundedRef,
  worktreeRef: BoundedRef,
  sessionRef: BoundedRef,
  attachmentGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  placementGeneration: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  grantRef: BoundedRef,
  attachedAt: Timestamp,
  expiresAt: S.NullOr(Timestamp),
});
const AdmissionRequestSchema = S.Struct({
  schemaVersion: S.Literal("openagents.desktop.ide-managed-sandbox.v1"),
  attachment: AttachmentSchema,
});
const CommandRequestSchema = S.Struct({
  schemaVersion: S.Literal("openagents.desktop.ide-managed-sandbox.v1"),
  command: ManagedSandboxCommandSchema,
  prompt: S.optionalKey(S.String.check(S.isMinLength(1), S.isMaxLength(100_000))),
  attachmentGeneration: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1))),
});

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

const errorResponse = (error: BoxV1FacadeError): Response =>
  json(
    {
      error: error.code,
      message: error.message,
      retryable: error.retryable,
    },
    { status: error.status },
  );

const parse = <A>(schema: S.Decoder<A>, request: Request) =>
  Effect.tryPromise({
    try: async () =>
      S.decodeUnknownSync(schema)(await request.json(), {
        onExcessProperty: "error",
      }),
    catch: () =>
      new BoxV1FacadeError({
        code: "validation_failed",
        status: 400,
        message: "request failed the managed-sandbox Desktop schema",
        retryable: false,
      }),
  });

const principal = (ownerUserId: string): BoxV1Principal => ({
  actorRef: "principal.desktop",
  ownerRef: ownerUserId,
  tenantRef: ownerUserId,
  login: "OpenAgents Desktop",
  email: null,
});

const responseFor = (
  result: Readonly<{
    command: ManagedSandboxCommand;
    resource: ManagedSandboxResource;
    receipt: unknown;
    turn: unknown;
    turnReceipt: unknown;
    events: unknown;
  }>,
) => ({ result });

export const makeManagedSandboxDesktopRoutes = <Bindings>(
  deps: ManagedSandboxDesktopRouteDependencies<Bindings>,
) => {
  const now = deps.now ?? (() => new Date());

  const authenticate = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    Effect.tryPromise({
      try: () => deps.authenticateOwner(request, env, ctx),
      catch: () =>
        new BoxV1FacadeError({
          code: "authentication_required",
          status: 401,
          message: "managed-sandbox owner authentication is unavailable",
          retryable: false,
        }),
    });

  const admission = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST") return json({ error: "method_not_allowed" }, { status: 405 });
      const owner = yield* authenticate(request, env, ctx);
      if (owner === undefined) return json({ error: "unauthorized" }, { status: 401 });
      const body = yield* parse(AdmissionRequestSchema, request);
      const checkedAt = now();
      if (!deps.enabled(env)) {
        return json(
          {
            admission: {
              _tag: "Unavailable",
              reason: "OpenAgents-managed placement is not admitted on the live target.",
              checkedAt: checkedAt.toISOString(),
            },
          },
          {},
          owner.decorateResponseHeaders,
        );
      }
      const policy = yield* deps.policy(env);
      const expiresAt = new Date(
        checkedAt.getTime() + policy.defaultTtlSeconds * 1_000,
      ).toISOString();
      const attachmentSuffix = body.attachment.agentAttachmentRef
        .replaceAll(/[^A-Za-z0-9_.:-]/gu, "_")
        .slice(-80);
      return json(
        {
          admission: {
            _tag: "Available",
            target: policy.target,
            imageDigest: policy.imageDigest,
            profileRef: policy.profileRef,
            lease: {
              leaseRef: `lease.desktop.sbx.${attachmentSuffix}`,
              state: "active",
              issuedAt: checkedAt.toISOString(),
              expiresAt,
              ttlSeconds: policy.defaultTtlSeconds,
              renewable: true,
            },
            budget: {
              currency: "USD",
              maxCostMicros: policy.maxCostMicros,
              maxCpuMillis: Math.min(policy.maxCpuMillis, policy.defaultTtlSeconds * 1_000),
              maxNetworkBytes: policy.maxNetworkBytes,
              maxArtifactBytes: policy.maxArtifactBytes,
              maxLifetimeSeconds: policy.defaultTtlSeconds,
            },
            requestedCapabilities: [
              "agent_turn",
              "command",
              "file_read",
              "file_write",
              "artifact_read",
            ].map((kind) => ({
              capabilityRef: `capability.desktop.sbx.${attachmentSuffix}.${kind}`,
              kind,
              state: "active",
              expiresAt,
            })),
            networkPosture: "deny_all",
            custody: "openagents_managed_region",
            retentionRef: "retention.managed-sandbox.phase1.v1",
            checkedAt: checkedAt.toISOString(),
          },
        },
        {},
        owner.decorateResponseHeaders,
      );
    }).pipe(Effect.catch((error) => Effect.succeed(errorResponse(error))));

  const commands = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST") return json({ error: "method_not_allowed" }, { status: 405 });
      const owner = yield* authenticate(request, env, ctx);
      if (owner === undefined) return json({ error: "unauthorized" }, { status: 401 });
      if (!deps.enabled(env)) {
        return json(
          {
            error: "runtime_not_admitted",
            message:
              "The managed-sandbox broker remains default-off until the live target gate passes.",
          },
          { status: 503 },
          owner.decorateResponseHeaders,
        );
      }
      const body = yield* parse(CommandRequestSchema, request);
      if (body.command.ownerRef !== owner.userId || body.command.tenantRef !== owner.userId) {
        return json(
          { error: "owner_scope_mismatch" },
          { status: 403 },
          owner.decorateResponseHeaders,
        );
      }
      const policy = yield* deps.policy(env);
      const runtime = yield* deps.runtime(env);
      const result = yield* makeManagedSandboxBroker({
        principal: principal(owner.userId),
        policy,
        runtime,
        store: deps.store(env),
        now,
      }).execute(body.command, {
        prompt: body.prompt,
        attachmentGeneration: body.attachmentGeneration,
      });
      return json(responseFor(result), {}, owner.decorateResponseHeaders);
    }).pipe(Effect.catch((error) => Effect.succeed(errorResponse(error))));

  return { admission, commands };
};
