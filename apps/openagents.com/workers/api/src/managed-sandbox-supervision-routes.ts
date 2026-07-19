import {
  decodeManagedSandboxSupervisionCommand,
  decodeManagedSandboxSupervisionEnvelope,
  decodeManagedSandboxSupervisionOutcome,
  type ManagedSandboxCommand,
  type ManagedSandboxEvent,
  type ManagedSandboxResource,
  ManagedSandboxSupervisionActorSchema,
  type ManagedSandboxSupervisionCommand,
  type ManagedSandboxSupervisionProjection,
  type ManagedSandboxTurn,
  type ManagedSandboxTurnReceipt,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import { makeManagedSandboxBroker } from "./managed-sandbox-broker";
import type {
  BoxV1NativeStore,
  BoxV1Policy,
  BoxV1Principal,
  BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import { BoxV1FacadeError } from "./managed-sandbox-box-v1-routes";

export const MANAGED_SANDBOX_MOBILE_SUPERVISION_PATH =
  "/api/managed-sandboxes/mobile/supervision" as const;
export const MANAGED_SANDBOX_WEB_SUPERVISION_PATH =
  "/api/managed-sandboxes/web/supervision" as const;

type Surface = "mobile" | "web";
type AuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ManagedSandboxSupervisionRouteDependencies<Bindings> = Readonly<{
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

const actorRef = (
  requestedByRef: string | undefined,
): typeof ManagedSandboxSupervisionActorSchema.Type => {
  switch (requestedByRef) {
    case "principal.desktop":
    case "principal.mobile":
    case "principal.sarah":
    case "principal.web":
    case "principal.system":
      return requestedByRef;
    default:
      return "principal.unknown";
  }
};

const elapsedSeconds = (from: string, now: Date): number =>
  Math.max(0, Math.floor((now.valueOf() - Date.parse(from)) / 1_000));

const structuralEvent = (events: ReadonlyArray<ManagedSandboxEvent>): ManagedSandboxEvent | null =>
  [...events].reverse().find((event) => event._tag !== "RuntimeTextDelta") ?? null;

const cleanupFor = (
  resource: ManagedSandboxResource,
): ManagedSandboxSupervisionProjection["cleanup"] => {
  if (resource.facts.cleanupComplete) return { state: "complete", receiptRef: null };
  if (resource.facts.lifecycle === "recovery_required") {
    return { state: "recovery_required", receiptRef: null };
  }
  if (["deleting", "deleted"].includes(resource.facts.lifecycle)) {
    return { state: "in_progress", receiptRef: null };
  }
  return { state: "not_started", receiptRef: null };
};

const attentionFor = (
  resource: ManagedSandboxResource,
): ManagedSandboxSupervisionProjection["attention"] => {
  if (resource.facts.lifecycle === "recovery_required") {
    return { state: "recovery_required", reasonRef: "reason.recovery_required" };
  }
  if (resource.facts.lifecycle === "failed") {
    return { state: "needs_action", reasonRef: "reason.runtime_failed" };
  }
  if (resource.lease.state === "expired") {
    return { state: "needs_action", reasonRef: "reason.lease_expired" };
  }
  return { state: "none", reasonRef: null };
};

const runtimeProjection = (
  turn: ManagedSandboxTurn | null,
  actor: typeof ManagedSandboxSupervisionActorSchema.Type,
): ManagedSandboxSupervisionProjection["runtime"] =>
  turn === null
    ? null
    : {
        turnRef: turn.turnRef,
        status: turn.status,
        identity: turn.runtime,
        actorRef: actor,
        startedAt: turn.startedAt ?? null,
        settledAt: turn.settledAt ?? null,
        terminalReasonRef:
          turn.terminalReason === undefined ? null : `reason.${turn.terminalReason}`,
      };

export const projectManagedSandboxSupervision = (
  input: Readonly<{
    resource: ManagedSandboxResource;
    events: ReadonlyArray<ManagedSandboxEvent>;
    latestTurn: ManagedSandboxTurn | null;
    latestTurnReceipt: ManagedSandboxTurnReceipt | null;
    latestActorRef: string | undefined;
    now: Date;
  }>,
): ManagedSandboxSupervisionProjection => {
  const event = structuralEvent(input.events);
  const receiptRefs = input.latestTurnReceipt === null ? [] : [input.latestTurnReceipt.receiptRef];
  const evidenceRefs = input.latestTurnReceipt?.evidenceRefs ?? [];
  return decodeManagedSandboxSupervisionEnvelope({
    projections: [
      {
        schema: "openagents.managed_sandbox_supervision.v1",
        sandboxRef: input.resource.sandboxRef,
        workUnitRef: input.resource.workUnitRef,
        attachmentRef: input.resource.attachmentRef,
        attachmentGeneration: input.resource.attachmentGeneration,
        resourceGeneration: input.resource.resourceGeneration,
        version: input.resource.version,
        target: {
          targetRef: input.resource.target.targetRef,
          provider: input.resource.target.provider,
          region: input.resource.target.region,
          isolation: input.resource.target.isolation,
          custody: input.resource.target.dataPosture,
        },
        state: {
          lifecycle: input.resource.facts.lifecycle,
          runtime: input.resource.facts.runtimeState,
          acceptingWork: input.resource.facts.acceptingWork,
        },
        timing: {
          createdAt: input.resource.createdAt,
          updatedAt: input.resource.updatedAt,
          leaseExpiresAt: input.resource.lease.expiresAt,
          elapsedSeconds: elapsedSeconds(input.resource.createdAt, input.now),
          idleSeconds: elapsedSeconds(input.resource.updatedAt, input.now),
          leaseState: input.resource.lease.state,
        },
        budget: {
          class: "bounded",
          currency: input.resource.budget.currency,
          maxCostMicros: input.resource.budget.maxCostMicros,
          observedCostMicros: null,
          state: "unreported",
        },
        runtime: runtimeProjection(input.latestTurn, actorRef(input.latestActorRef)),
        lastStructuralEvent:
          event === null
            ? null
            : {
                eventRef: event.eventRef,
                kind: event._tag,
                sequence: event.sequence,
                observedAt: event.observedAt,
              },
        attention: attentionFor(input.resource),
        cleanup: cleanupFor(input.resource),
        outcomes: {
          fileRefs: [],
          changeRefs: [],
          artifactRefs: [],
          evidenceRefs,
          receiptRefs,
        },
      },
    ],
    observedAt: input.now.toISOString(),
  }).projections[0]!;
};

const projectResource = (
  store: BoxV1NativeStore,
  resource: ManagedSandboxResource,
  now: Date,
): Effect.Effect<ManagedSandboxSupervisionProjection, BoxV1FacadeError> =>
  Effect.gen(function* () {
    const eventPage =
      resource.lastEventSequence === 0
        ? null
        : yield* store.readEvents({
            ownerRef: resource.ownerRef,
            tenantRef: resource.tenantRef,
            sandboxRef: resource.sandboxRef,
            afterSequence: Math.max(0, resource.lastEventSequence - 1_000),
            limit: Math.min(1_000, resource.lastEventSequence),
          });
    const turns = yield* store.turns({
      ownerRef: resource.ownerRef,
      tenantRef: resource.tenantRef,
      sandboxRef: resource.sandboxRef,
    });
    const latestOrder = turns.at(-1);
    const latest =
      latestOrder === undefined
        ? null
        : yield* store.inspectTurn({
            ownerRef: resource.ownerRef,
            tenantRef: resource.tenantRef,
            sandboxRef: resource.sandboxRef,
            turnRef: latestOrder.turnRef,
          });
    const reservation =
      latest === null
        ? undefined
        : yield* store.reservation({
            ownerRef: resource.ownerRef,
            tenantRef: resource.tenantRef,
            commandRef: latest.turn.commandRef,
          });
    return projectManagedSandboxSupervision({
      resource,
      events: eventPage?.events ?? [],
      latestTurn: latest?.turn ?? null,
      latestTurnReceipt: latest?.receipt ?? null,
      latestActorRef: reservation?.command.requestedByRef,
      now,
    });
  });

const nativeCommand = (
  command: ManagedSandboxSupervisionCommand,
  ownerRef: string,
): ManagedSandboxCommand => {
  const base = {
    schema: "openagents.managed_sandbox_command.v1" as const,
    commandRef: command.commandRef,
    requestedByRef: `principal.${command.surface}`,
    ownerRef,
    tenantRef: ownerRef,
    idempotencyRef: command.idempotencyRef,
    requestedAt: command.issuedAt,
    sandboxRef: command.sandboxRef,
    expectedVersion: command.expectedVersion,
  };
  switch (command._tag) {
    case "Interrupt":
      return {
        _tag: "Interrupt",
        ...base,
        turnRef: command.turnRef,
        reasonRef: command.reasonRef,
      };
    case "Stop":
      return { _tag: "Stop", ...base, reasonRef: command.reasonRef };
    case "Resume":
      return { _tag: "Resume", ...base };
    case "Delete":
      return { _tag: "Delete", ...base, reasonRef: command.reasonRef };
  }
};

const principal = (surface: Surface, ownerRef: string): BoxV1Principal => ({
  actorRef: `principal.${surface}`,
  ownerRef,
  tenantRef: ownerRef,
  login: surface === "mobile" ? "OpenAgents Mobile" : "OpenAgents Web",
  email: null,
});

const safeReason = (error: BoxV1FacadeError): string => {
  switch (error.code) {
    case "authentication_required":
      return "reason.authentication_required";
    case "permission_denied":
      return "reason.permission_denied";
    case "resource_not_found":
      return "reason.resource_not_found";
    case "conflict":
      return "reason.conflict";
    case "validation_failed":
      return "reason.validation_failed";
    default:
      return error.retryable ? "reason.temporarily_unavailable" : "reason.operation_failed";
  }
};

export const makeManagedSandboxSupervisionRoutes = <Bindings>(
  deps: ManagedSandboxSupervisionRouteDependencies<Bindings>,
) => {
  const now = deps.now ?? (() => new Date());

  const authenticate = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    Effect.tryPromise({
      try: () => deps.authenticateOwner(request, env, ctx),
      catch: () => undefined,
    });

  const handle = (
    surface: Surface,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> =>
    Effect.gen(function* () {
      if (request.method !== "GET" && request.method !== "POST") {
        return json({ error: "method_not_allowed" }, { status: 405 });
      }
      const owner = yield* authenticate(request, env, ctx);
      if (owner === undefined) return json({ error: "unauthorized" }, { status: 401 });
      if (!deps.enabled(env)) {
        return json(
          { error: "runtime_not_admitted", retryable: true },
          { status: 503 },
          owner.decorateResponseHeaders,
        );
      }
      const store = deps.store(env);
      const observedAt = now();
      if (request.method === "GET") {
        const resources = yield* store.list({
          ownerRef: owner.userId,
          tenantRef: owner.userId,
          limit: 100,
        });
        const projections = yield* Effect.forEach(
          resources,
          (resource) => projectResource(store, resource, observedAt),
          { concurrency: 8 },
        );
        return json(
          decodeManagedSandboxSupervisionEnvelope({
            projections,
            observedAt: observedAt.toISOString(),
          }),
          {},
          owner.decorateResponseHeaders,
        );
      }

      const body = yield* Effect.tryPromise({
        try: async () => (await request.json()) as { command?: unknown },
        catch: () => ({ command: undefined }),
      });
      const decodedCommand = yield* Effect.try({
        try: () => decodeManagedSandboxSupervisionCommand(body.command),
        catch: () => "validation_failed" as const,
      }).pipe(Effect.option);
      if (decodedCommand._tag === "None") {
        return json({ error: "validation_failed" }, { status: 400 }, owner.decorateResponseHeaders);
      }
      const command: ManagedSandboxSupervisionCommand = decodedCommand.value;
      if (command.surface !== surface) {
        return json({ error: "surface_mismatch" }, { status: 403 }, owner.decorateResponseHeaders);
      }
      if (Date.parse(command.expiresAt) <= observedAt.valueOf()) {
        return json(
          decodeManagedSandboxSupervisionOutcome({
            schema: "openagents.managed_sandbox_supervision_outcome.v1",
            commandRef: command.commandRef,
            idempotencyRef: command.idempotencyRef,
            state: "refused",
            reasonRef: "reason.command_expired",
            receiptRefs: [],
            projection: null,
            observedAt: observedAt.toISOString(),
          }),
          { status: 409 },
          owner.decorateResponseHeaders,
        );
      }
      const current = yield* store.inspect({
        ownerRef: owner.userId,
        tenantRef: owner.userId,
        sandboxRef: command.sandboxRef,
      });
      if (current.resourceGeneration !== command.expectedResourceGeneration) {
        return json(
          decodeManagedSandboxSupervisionOutcome({
            schema: "openagents.managed_sandbox_supervision_outcome.v1",
            commandRef: command.commandRef,
            idempotencyRef: command.idempotencyRef,
            state: "refused",
            reasonRef: "reason.stale_generation",
            receiptRefs: [],
            projection: yield* projectResource(store, current, observedAt),
            observedAt: observedAt.toISOString(),
          }),
          { status: 409 },
          owner.decorateResponseHeaders,
        );
      }
      const policy = yield* deps.policy(env);
      const runtime = yield* deps.runtime(env);
      const broker = makeManagedSandboxBroker({
        principal: principal(surface, owner.userId),
        policy,
        store,
        runtime,
        now,
      });
      const result = yield* broker.execute(nativeCommand(command, owner.userId));
      const projection = yield* projectResource(store, result.resource, now());
      const state =
        result.receipt.outcome === "failed"
          ? "failed"
          : result.receipt.outcome === "refused"
            ? "refused"
            : result.receipt.outcome === "accepted"
              ? "pending"
              : "applied";
      return json(
        decodeManagedSandboxSupervisionOutcome({
          schema: "openagents.managed_sandbox_supervision_outcome.v1",
          commandRef: command.commandRef,
          idempotencyRef: command.idempotencyRef,
          state,
          reasonRef:
            result.receipt.errorCode === undefined ? null : `reason.${result.receipt.errorCode}`,
          receiptRefs: [result.receipt.receiptRef],
          projection,
          observedAt: now().toISOString(),
        }),
        {},
        owner.decorateResponseHeaders,
      );
    }).pipe(
      Effect.catch((error) => {
        const facade =
          error instanceof BoxV1FacadeError
            ? error
            : new BoxV1FacadeError({
                code: "upstream_unavailable",
                status: 503,
                message: "managed-sandbox supervision is unavailable",
                retryable: true,
              });
        return Effect.succeed(
          json(
            {
              error: facade.code,
              reasonRef: safeReason(facade),
              retryable: facade.retryable,
            },
            { status: facade.status },
          ),
        );
      }),
    );

  return {
    mobile: (request: Request, env: Bindings, ctx: ExecutionContext) =>
      handle("mobile", request, env, ctx),
    web: (request: Request, env: Bindings, ctx: ExecutionContext) =>
      handle("web", request, env, ctx),
  };
};
