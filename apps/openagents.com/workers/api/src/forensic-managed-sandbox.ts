import {
  FORENSIC_WORKER_PLACEMENT_VERSION,
  type ForensicBudget,
  ForensicBudgetSchema,
  type ForensicWorkerPlacement,
  ForensicWorkerPlacementSchema,
} from "@openagentsinc/forensic-contract";
import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  type ManagedSandboxCommand,
  ManagedSandboxCommandSchema,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import {
  BoxV1FacadeError,
  type BoxV1NativeStore,
  type BoxV1Policy,
  type BoxV1Principal,
  type BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import {
  type ManagedSandboxBroker,
  type ManagedSandboxBrokerResult,
  makeManagedSandboxBroker,
} from "./managed-sandbox-broker";

const FORENSIC_DRIVER_REF = "driver.openagents.forensic-worker.v1";
const FORENSIC_PROFILE_REF = "profile.sbx.gce.e2-small.v1";
const FORENSIC_NETWORK_POLICY_REF =
  "network-policy-ref://openagents/managed-sandbox/broker-only-v1";

export const FORENSIC_MANAGED_SANDBOX_PATH = "/api/forensics/workers" as const;
const INITIAL_FORENSIC_BUDGET_REF = "budget.forensic.worker.initial.v1";

export type ForensicWorkerUsage = Readonly<{
  elapsedSeconds: number;
  tokens: number;
  costMicros: number;
  artifactBytes: number;
  networkBytes: number;
  activeTurns: number;
  costMeasured: boolean;
}>;

export type ForensicWorkerAdmission = Readonly<{
  commandRef: string;
  idempotencyRef: string;
  requestedByRef: string;
  ownerRef: string;
  tenantRef: string;
  workUnitRef: string;
  attachmentRef: string;
  placementRef: string;
  regionRef: string;
  budgetRef: string;
  requestedAt: string;
  expiresAt: string;
  budget: ForensicBudget;
}>;

export type ForensicWorkerDispatch = Readonly<{
  commandRef: string;
  idempotencyRef: string;
  requestedByRef: string;
  turnRef: string;
  capabilityRef: string;
  requestedAt: string;
  prompt: string;
  usage: ForensicWorkerUsage;
}>;

export type ForensicWorkerCancellation = Readonly<{
  commandRef: string;
  inspectCommandRef: string;
  idempotencyRef: string;
  inspectIdempotencyRef: string;
  requestedByRef: string;
  turnRef: string;
  reasonRef: string;
  requestedAt: string;
}>;

export type ForensicManagedSandbox = Readonly<{
  admit: (
    admission: ForensicWorkerAdmission,
  ) => Effect.Effect<ForensicWorkerPlacement, BoxV1FacadeError>;
  dispatch: (
    placement: ForensicWorkerPlacement,
    request: ForensicWorkerDispatch,
  ) => Effect.Effect<ManagedSandboxBrokerResult, BoxV1FacadeError>;
  cancel: (
    placement: ForensicWorkerPlacement,
    request: ForensicWorkerCancellation,
  ) => Effect.Effect<ManagedSandboxBrokerResult, BoxV1FacadeError>;
  delete: (
    placement: ForensicWorkerPlacement,
    request: Readonly<{
      commandRef: string;
      idempotencyRef: string;
      requestedByRef: string;
      reasonRef: string;
      requestedAt: string;
    }>,
  ) => Effect.Effect<ForensicWorkerPlacement, BoxV1FacadeError>;
}>;

const refuse = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "conflict",
    status: 409,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  });

const recoveryRequired = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "upstream_unavailable",
    status: 503,
    message,
    retryable: true,
    ...(details === undefined ? {} : { details }),
  });

const decodeCommand = (value: unknown) =>
  Effect.try({
    try: () => S.decodeUnknownSync(ManagedSandboxCommandSchema)(value),
    catch: (error) => refuse("forensic worker command failed strict validation", error),
  });

const decodePlacement = (value: unknown) =>
  Effect.try({
    try: () => S.decodeUnknownSync(ForensicWorkerPlacementSchema)(value),
    catch: (error) => recoveryRequired("forensic placement receipt failed validation", error),
  });

const allTrue = (proof: Readonly<Record<string, boolean>> | undefined): boolean =>
  proof !== undefined && Object.values(proof).every(Boolean);

const assertOrderedEvents = (result: ManagedSandboxBrokerResult) => {
  const ordered = result.events.every(
    (event, index) => index === 0 || event.sequence > result.events[index - 1]!.sequence,
  );
  return ordered
    ? Effect.void
    : Effect.fail(recoveryRequired("forensic runtime emitted unordered receipts"));
};

export const assertForensicBudgetBelowPrompt = (
  budget: ForensicBudget,
  usage: ForensicWorkerUsage,
): Effect.Effect<void, BoxV1FacadeError> => {
  if (!usage.costMeasured) {
    return Effect.fail(refuse("forensic dispatch requires measured cost truth"));
  }
  if (budget.maxConcurrency !== 1 || usage.activeTurns >= budget.maxConcurrency) {
    return Effect.fail(refuse("forensic dispatch exceeds its single-turn concurrency budget"));
  }
  if (
    usage.elapsedSeconds >= budget.maxTimeSeconds ||
    usage.tokens >= budget.maxTokens ||
    usage.costMicros >= budget.maxCostMicros ||
    usage.artifactBytes >= budget.maxArtifactBytes ||
    usage.networkBytes >= budget.maxNetworkBytes
  ) {
    return Effect.fail(refuse("forensic dispatch has no remaining admitted budget"));
  }
  return Effect.void;
};

const sha256 = (value: string): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
      return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    },
    catch: (error) => recoveryRequired("forensic digest computation failed", error),
  });

export const makeForensicManagedSandbox = (
  input: Readonly<{
    broker: ManagedSandboxBroker;
    policy: BoxV1Policy;
    profileDigest: `sha256:${string}`;
    resolveBudget: (budgetRef: string) => Effect.Effect<ForensicBudget, BoxV1FacadeError>;
  }>,
): ForensicManagedSandbox => {
  const assertForensicPolicy = () => {
    const target = input.policy.target;
    if (
      target.targetClass !== "openagents_managed" ||
      target.provider !== "google_cloud" ||
      target.adapterRef !== "adapter.oa-codex-control.gce.v1" ||
      target.isolation !== "gce_vm" ||
      target.dataPosture !== "openagents_managed_region" ||
      input.policy.profileRef !== FORENSIC_PROFILE_REF
    ) {
      return Effect.fail(
        refuse("forensic work requires the exact OpenAgents Cloud GCE target and profile"),
      );
    }
    return Effect.void;
  };

  const admit: ForensicManagedSandbox["admit"] = (admission) =>
    Effect.gen(function* () {
      yield* assertForensicPolicy();
      if (admission.budget.maxConcurrency !== 1) {
        return yield* refuse("initial forensic workers admit exactly one active turn");
      }
      const authoritativeBudget = yield* input.resolveBudget(admission.budgetRef);
      if (canonicalJson(authoritativeBudget) !== canonicalJson(admission.budget)) {
        return yield* refuse("forensic admission budget does not match budget authority");
      }
      const ttlSeconds = Math.floor(
        (Date.parse(admission.expiresAt) - Date.parse(admission.requestedAt)) / 1_000,
      );
      if (!Number.isSafeInteger(ttlSeconds) || ttlSeconds < 1) {
        return yield* refuse("forensic worker lease is invalid");
      }
      const capabilityRef = `capability.${admission.workUnitRef}.agent_turn`;
      const command = yield* decodeCommand({
        _tag: "Create",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: admission.commandRef,
        requestedByRef: admission.requestedByRef,
        ownerRef: admission.ownerRef,
        tenantRef: admission.tenantRef,
        idempotencyRef: admission.idempotencyRef,
        requestedAt: admission.requestedAt,
        workUnitRef: admission.workUnitRef,
        attachmentRef: admission.attachmentRef,
        target: input.policy.target,
        imageDigest: input.policy.imageDigest,
        profileRef: input.policy.profileRef,
        lease: {
          leaseRef: `lease.${admission.workUnitRef}`,
          state: "active",
          issuedAt: admission.requestedAt,
          expiresAt: admission.expiresAt,
          ttlSeconds,
          renewable: false,
        },
        budget: {
          currency: "USD",
          maxCostMicros: admission.budget.maxCostMicros,
          maxCpuMillis: admission.budget.maxTimeSeconds * 1_000,
          maxNetworkBytes: admission.budget.maxNetworkBytes,
          maxArtifactBytes: admission.budget.maxArtifactBytes,
          maxLifetimeSeconds: admission.budget.maxTimeSeconds,
        },
        requestedCapabilities: [
          {
            capabilityRef,
            kind: "agent_turn",
            state: "active",
            expiresAt: admission.expiresAt,
          },
        ],
      });
      const result = yield* input.broker.execute(command, {
        attachmentGeneration: 1,
      });
      const outcome = result.lifecycleOutcome;
      if (
        result.receipt.outcome !== "succeeded" ||
        outcome?.phase !== "ready" ||
        outcome.forensicDriverRef !== FORENSIC_DRIVER_REF ||
        !allTrue(outcome.readinessProof) ||
        outcome.measuredCostMicros > admission.budget.maxCostMicros
      ) {
        return yield* recoveryRequired("forensic worker readiness or cost truth was incomplete");
      }
      yield* assertOrderedEvents(result);
      return yield* decodePlacement({
        schema: FORENSIC_WORKER_PLACEMENT_VERSION,
        placementRef: admission.placementRef,
        ownerRef: admission.ownerRef,
        tenantRef: admission.tenantRef,
        workUnitRef: admission.workUnitRef,
        sandboxRef: result.resource.sandboxRef,
        attachmentGeneration: result.resource.attachmentGeneration,
        resourceGeneration: result.resource.resourceGeneration,
        targetClass: "openagents_managed",
        provider: "google_cloud",
        adapterRef: "adapter.oa-codex-control.gce.v1",
        isolation: "gce_vm",
        regionRef: admission.regionRef,
        imageDigest: input.policy.imageDigest,
        profileDigest: input.profileDigest,
        networkPolicyRef: FORENSIC_NETWORK_POLICY_REF,
        leaseRef: result.resource.lease.leaseRef,
        budgetRef: admission.budgetRef,
        capabilityRefs: [capabilityRef],
        state: "worker_ready",
        admissionReceiptRef: result.receipt.receiptRef,
        readinessReceiptRef: outcome.receiptRef,
        updatedAt: outcome.observedAt,
      });
    });

  const dispatch: ForensicManagedSandbox["dispatch"] = (placement, request) =>
    Effect.gen(function* () {
      const resources = yield* input.broker.list();
      const resource = resources.find((candidate) => candidate.sandboxRef === placement.sandboxRef);
      if (resource === undefined || resource.facts.lifecycle !== "ready") {
        return yield* recoveryRequired("forensic worker is not ready for dispatch");
      }
      const budget = yield* input.resolveBudget(placement.budgetRef);
      yield* assertForensicBudgetBelowPrompt(budget, request.usage);
      if (
        budget.maxTimeSeconds !== resource.budget.maxLifetimeSeconds ||
        budget.maxCostMicros !== resource.budget.maxCostMicros ||
        budget.maxArtifactBytes !== resource.budget.maxArtifactBytes ||
        budget.maxNetworkBytes !== resource.budget.maxNetworkBytes
      ) {
        return yield* refuse("forensic dispatch budget does not bind the admitted worker");
      }
      const promptDigest = yield* sha256(request.prompt);
      const command = yield* decodeCommand({
        _tag: "Dispatch",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.commandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.idempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        turnRef: request.turnRef,
        capabilityRef: request.capabilityRef,
        promptDigest: `sha256:${promptDigest}`,
        runtime: {
          provider: "codex",
          modelRef: "model.openai.gpt-5",
          harnessRef: FORENSIC_DRIVER_REF,
        },
      });
      const result = yield* input.broker.execute(command, {
        prompt: request.prompt,
      });
      yield* assertOrderedEvents(result);
      return result;
    });

  const cancel: ForensicManagedSandbox["cancel"] = (placement, request) =>
    Effect.gen(function* () {
      const resources = yield* input.broker.list();
      const resource = resources.find((candidate) => candidate.sandboxRef === placement.sandboxRef);
      if (resource === undefined) return yield* recoveryRequired("forensic worker is missing");
      const interrupt = yield* decodeCommand({
        _tag: "Interrupt",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.commandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.idempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        turnRef: request.turnRef,
        reasonRef: request.reasonRef,
      });
      const interrupted = yield* input.broker.execute(interrupt);
      yield* assertOrderedEvents(interrupted);
      const inspect = yield* decodeCommand({
        _tag: "Inspect",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.inspectCommandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.inspectIdempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
      });
      const settled = yield* input.broker.execute(inspect);
      yield* assertOrderedEvents(settled);
      if (
        settled.turn === null ||
        !["settled", "failed", "interrupted"].includes(settled.turn.status)
      ) {
        return yield* recoveryRequired("forensic cancellation did not structurally settle");
      }
      return settled;
    });

  const remove: ForensicManagedSandbox["delete"] = (placement, request) =>
    Effect.gen(function* () {
      const resources = yield* input.broker.list();
      const resource = resources.find((candidate) => candidate.sandboxRef === placement.sandboxRef);
      if (resource === undefined) return yield* recoveryRequired("forensic worker is missing");
      const command: ManagedSandboxCommand = yield* decodeCommand({
        _tag: "Delete",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: request.commandRef,
        requestedByRef: request.requestedByRef,
        ownerRef: placement.ownerRef,
        tenantRef: placement.tenantRef,
        idempotencyRef: request.idempotencyRef,
        requestedAt: request.requestedAt,
        sandboxRef: placement.sandboxRef,
        expectedVersion: resource.version,
        reasonRef: request.reasonRef,
      });
      const result = yield* input.broker.execute(command);
      const outcome = result.lifecycleOutcome;
      if (
        result.receipt.outcome !== "succeeded" ||
        outcome?.phase !== "deleted" ||
        outcome.forensicDriverRef !== FORENSIC_DRIVER_REF ||
        !allTrue(outcome.cleanupProof)
      ) {
        return yield* recoveryRequired("forensic worker deletion left incomplete cleanup truth");
      }
      yield* assertOrderedEvents(result);
      return yield* decodePlacement({
        ...placement,
        resourceGeneration: result.resource.resourceGeneration,
        state: "cleaned",
        deletionReceiptRef: result.receipt.receiptRef,
        cleanupReceiptRef: outcome.receiptRef,
        updatedAt: outcome.observedAt,
      });
    });

  return { admit, dispatch, cancel, delete: remove };
};

const NonNegativeInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
const ForensicRouteTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/u),
);

const ForensicUsageSchema = S.Struct({
  elapsedSeconds: NonNegativeInteger,
  tokens: NonNegativeInteger,
  costMicros: NonNegativeInteger,
  artifactBytes: NonNegativeInteger,
  networkBytes: NonNegativeInteger,
  activeTurns: NonNegativeInteger,
  costMeasured: S.Boolean,
});

const ForensicWorkerRouteRequestSchema = S.TaggedUnion({
  Admit: {
    commandRef: S.String,
    idempotencyRef: S.String,
    workUnitRef: S.String,
    attachmentRef: S.String,
    placementRef: S.String,
    regionRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
  Dispatch: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    idempotencyRef: S.String,
    turnRef: S.String,
    capabilityRef: S.String,
    prompt: S.String.check(S.isMinLength(1), S.isMaxLength(100_000)),
    usage: ForensicUsageSchema,
    requestedAt: ForensicRouteTimestamp,
  },
  Cancel: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    inspectCommandRef: S.String,
    idempotencyRef: S.String,
    inspectIdempotencyRef: S.String,
    turnRef: S.String,
    reasonRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
  Delete: {
    placement: ForensicWorkerPlacementSchema,
    commandRef: S.String,
    idempotencyRef: S.String,
    reasonRef: S.String,
    requestedAt: ForensicRouteTimestamp,
  },
});

type ForensicAuthenticatedOwner = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ForensicManagedSandboxRouteDependencies<Bindings> = Readonly<{
  authenticateOwner: (
    request: Request,
    env: Bindings,
    context: ExecutionContext,
  ) => Promise<ForensicAuthenticatedOwner | undefined>;
  enabled: (env: Bindings) => boolean;
  policy: (env: Bindings) => Effect.Effect<BoxV1Policy, BoxV1FacadeError>;
  profileDigest: (env: Bindings) => string | undefined;
  store: (env: Bindings) => BoxV1NativeStore;
  runtime: (env: Bindings) => Effect.Effect<BoxV1Runtime, BoxV1FacadeError>;
  now?: (() => Date) | undefined;
}>;

const routeJson = (
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

const routeError = (error: BoxV1FacadeError): Response =>
  routeJson(
    { error: error.code, message: error.message, retryable: error.retryable },
    { status: error.status },
  );

const initialForensicBudget = (policy: BoxV1Policy): ForensicBudget => ({
  maxTimeSeconds: policy.defaultTtlSeconds,
  maxTokens: 200_000,
  maxCostMicros: policy.maxCostMicros,
  maxConcurrency: 1,
  maxArtifactBytes: policy.maxArtifactBytes,
  maxNetworkBytes: policy.maxNetworkBytes,
});

export const makeForensicManagedSandboxRoutes = <Bindings>(
  dependencies: ForensicManagedSandboxRouteDependencies<Bindings>,
) => {
  const now = dependencies.now ?? (() => new Date());
  const handle = (request: Request, env: Bindings, context: ExecutionContext) =>
    Effect.gen(function* () {
      if (request.method !== "POST") {
        return routeJson({ error: "method_not_allowed" }, { status: 405 });
      }
      const owner = yield* Effect.tryPromise({
        try: () => dependencies.authenticateOwner(request, env, context),
        catch: () =>
          new BoxV1FacadeError({
            code: "authentication_required",
            status: 401,
            message: "forensic worker owner authentication is unavailable",
            retryable: false,
          }),
      });
      if (owner === undefined) return routeJson({ error: "unauthorized" }, { status: 401 });
      if (!dependencies.enabled(env)) {
        return routeJson(
          {
            error: "runtime_not_admitted",
            message: "The OpenAgents Cloud forensic worker remains default-off.",
          },
          { status: 503 },
          owner.decorateResponseHeaders,
        );
      }
      const body = yield* Effect.tryPromise({
        try: async () =>
          S.decodeUnknownSync(ForensicWorkerRouteRequestSchema)(await request.json(), {
            onExcessProperty: "error",
          }),
        catch: (error) =>
          new BoxV1FacadeError({
            code: "validation_failed",
            status: 400,
            message: "request failed the forensic worker schema",
            retryable: false,
            details: error,
          }),
      });
      const policy = yield* dependencies.policy(env);
      const profileDigest = dependencies.profileDigest(env);
      if (profileDigest === undefined || !/^sha256:[0-9a-f]{64}$/u.test(profileDigest)) {
        return yield* recoveryRequired("forensic worker profile digest is unavailable");
      }
      const budget = S.decodeUnknownSync(ForensicBudgetSchema)(initialForensicBudget(policy));
      const broker = makeManagedSandboxBroker({
        principal: {
          actorRef: `agent:${owner.userId}`,
          ownerRef: owner.userId,
          tenantRef: owner.userId,
          login: "OpenAgents Forensics",
          email: null,
        } satisfies BoxV1Principal,
        policy,
        store: dependencies.store(env),
        runtime: yield* dependencies.runtime(env),
        now,
      });
      const forensic = makeForensicManagedSandbox({
        broker,
        policy,
        profileDigest: profileDigest as `sha256:${string}`,
        resolveBudget: (budgetRef) =>
          budgetRef === INITIAL_FORENSIC_BUDGET_REF
            ? Effect.succeed(budget)
            : Effect.fail(refuse("forensic budget ref is not admitted")),
      });
      const requestedByRef = `agent:${owner.userId}`;
      const handledAtMs = now().getTime();
      let result: unknown;
      switch (body._tag) {
        case "Admit":
          if (
            Date.parse(body.requestedAt) > handledAtMs + 60_000 ||
            Date.parse(body.requestedAt) + budget.maxTimeSeconds * 1_000 <= handledAtMs
          ) {
            return yield* refuse("forensic admission timestamp is outside its lease window");
          }
          result = yield* forensic.admit({
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            ownerRef: owner.userId,
            tenantRef: owner.userId,
            workUnitRef: body.workUnitRef,
            attachmentRef: body.attachmentRef,
            placementRef: body.placementRef,
            regionRef: body.regionRef,
            budgetRef: INITIAL_FORENSIC_BUDGET_REF,
            requestedAt: body.requestedAt,
            expiresAt: new Date(
              Date.parse(body.requestedAt) + budget.maxTimeSeconds * 1_000,
            ).toISOString(),
            budget,
          });
          break;
        case "Dispatch":
          result = yield* forensic.dispatch(body.placement, {
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            turnRef: body.turnRef,
            capabilityRef: body.capabilityRef,
            requestedAt: body.requestedAt,
            prompt: body.prompt,
            usage: body.usage,
          });
          break;
        case "Cancel":
          result = yield* forensic.cancel(body.placement, {
            commandRef: body.commandRef,
            inspectCommandRef: body.inspectCommandRef,
            idempotencyRef: body.idempotencyRef,
            inspectIdempotencyRef: body.inspectIdempotencyRef,
            requestedByRef,
            turnRef: body.turnRef,
            reasonRef: body.reasonRef,
            requestedAt: body.requestedAt,
          });
          break;
        case "Delete":
          result = yield* forensic.delete(body.placement, {
            commandRef: body.commandRef,
            idempotencyRef: body.idempotencyRef,
            requestedByRef,
            reasonRef: body.reasonRef,
            requestedAt: body.requestedAt,
          });
          break;
      }
      return routeJson({ result }, {}, owner.decorateResponseHeaders);
    }).pipe(Effect.catch((error) => Effect.succeed(routeError(error))));

  return { handle };
};
