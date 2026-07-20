import type { ManagedSandboxCommandReservation } from "@openagentsinc/khala-sync-server";
import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  type ManagedSandboxCommand,
  ManagedSandboxCommandSchema,
  type ManagedSandboxEvent,
  ManagedSandboxEventSchema,
  type ManagedSandboxReceipt,
  ManagedSandboxReceiptSchema,
  type ManagedSandboxResource,
  ManagedSandboxResourceSchema,
  type ManagedSandboxRuntimeEventInput,
  type ManagedSandboxTurn,
  type ManagedSandboxTurnReceipt,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";

import {
  type BoxV1LifecycleOutcome,
  type BoxV1NativeStore,
  type BoxV1Policy,
  type BoxV1Principal,
  type BoxV1Runtime,
  BoxV1FacadeError,
} from "./managed-sandbox-box-v1-routes";

export type ManagedSandboxBrokerResult = Readonly<{
  command: ManagedSandboxCommand;
  resource: ManagedSandboxResource;
  receipt: ManagedSandboxReceipt;
  turn: ManagedSandboxTurn | null;
  turnReceipt: ManagedSandboxTurnReceipt | null;
  events: ReadonlyArray<ManagedSandboxEvent>;
}>;

export type ManagedSandboxBroker = Readonly<{
  execute: (
    command: ManagedSandboxCommand,
    options?: Readonly<{
      prompt?: string | undefined;
      attachmentGeneration?: number | undefined;
      initialResource?: ManagedSandboxResource | undefined;
    }>,
  ) => Effect.Effect<ManagedSandboxBrokerResult, BoxV1FacadeError>;
  list: (limit?: number) => Effect.Effect<ReadonlyArray<ManagedSandboxResource>, BoxV1FacadeError>;
}>;

const invalid = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "validation_failed",
    status: 400,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  });

const conflict = (message: string, details?: unknown) =>
  new BoxV1FacadeError({
    code: "conflict",
    status: 409,
    message,
    retryable: false,
    ...(details === undefined ? {} : { details }),
  });

const unavailable = (capability: string) =>
  new BoxV1FacadeError({
    code: "upstream_unavailable",
    status: 503,
    message: `${capability} is unavailable on the admitted managed-sandbox target`,
    retryable: true,
    details: { capability },
  });

const digest = (value: string): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
      return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    },
    catch: () => unavailable("request_identity"),
  });

const decode = <A>(schema: S.Decoder<A>, value: unknown): Effect.Effect<A, BoxV1FacadeError> =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value, { onExcessProperty: "error" }),
    catch: (error) => invalid("managed-sandbox broker value failed schema validation", error),
  });

const sameTarget = (
  left: ManagedSandboxResource["target"],
  right: ManagedSandboxResource["target"],
): boolean =>
  left.targetRef === right.targetRef &&
  left.targetClass === right.targetClass &&
  left.provider === right.provider &&
  left.adapterRef === right.adapterRef &&
  left.region === right.region &&
  left.isolation === right.isolation &&
  left.dataPosture === right.dataPosture;

const assertPolicy = (
  policy: BoxV1Policy,
  input: Readonly<{
    target: ManagedSandboxResource["target"];
    imageDigest: string;
    profileRef: string;
    lease: ManagedSandboxResource["lease"];
    budget: ManagedSandboxResource["budget"];
    capabilities: ManagedSandboxResource["capabilities"];
  }>,
): Effect.Effect<void, BoxV1FacadeError> => {
  if (
    !sameTarget(input.target, policy.target) ||
    input.imageDigest !== policy.imageDigest ||
    input.profileRef !== policy.profileRef
  ) {
    return Effect.fail(
      conflict("request does not bind the exact admitted target, image, and profile"),
    );
  }
  if (
    input.lease.ttlSeconds > policy.maxTtlSeconds ||
    input.budget.maxLifetimeSeconds > input.lease.ttlSeconds ||
    input.budget.maxCostMicros > policy.maxCostMicros ||
    input.budget.maxCpuMillis > policy.maxCpuMillis ||
    input.budget.maxNetworkBytes > policy.maxNetworkBytes ||
    input.budget.maxArtifactBytes > policy.maxArtifactBytes
  ) {
    return Effect.fail(conflict("request exceeds the admitted lease or budget"));
  }
  if (
    input.capabilities.length === 0 ||
    input.capabilities.some(
      (capability) =>
        capability.state !== "active" ||
        Date.parse(capability.expiresAt) > Date.parse(input.lease.expiresAt),
    )
  ) {
    return Effect.fail(conflict("request capabilities are not active and lease-bounded"));
  }
  return Effect.void;
};

const syntheticReceipt = (
  reservation: ManagedSandboxCommandReservation,
): Effect.Effect<ManagedSandboxReceipt, BoxV1FacadeError> =>
  reservation.receipt !== undefined
    ? Effect.succeed(reservation.receipt)
    : digest(reservation.command.commandRef).pipe(
        Effect.flatMap((value) =>
          decode(ManagedSandboxReceiptSchema, {
            schema: "openagents.managed_sandbox_receipt.v1",
            receiptRef: `receipt.sbx.broker.${value.slice(0, 40)}`,
            commandRef: reservation.command.commandRef,
            sandboxRef: reservation.resource.sandboxRef,
            ownerRef: reservation.resource.ownerRef,
            tenantRef: reservation.resource.tenantRef,
            resourceGeneration: reservation.resource.resourceGeneration,
            version: reservation.resource.version,
            outcome: "accepted",
            lifecycle: reservation.resource.facts.lifecycle,
            eventRefs: [],
            artifactRefs: [],
            observedAt: reservation.command.requestedAt,
          }),
        ),
      );

const initialResource = (
  command: Extract<ManagedSandboxCommand, { _tag: "Create" }>,
  sandboxRef: string,
  attachmentGeneration: number,
): Effect.Effect<ManagedSandboxResource, BoxV1FacadeError> =>
  decode(ManagedSandboxResourceSchema, {
    schema: "openagents.managed_sandbox.v1",
    sandboxRef,
    ownerRef: command.ownerRef,
    tenantRef: command.tenantRef,
    programRef: "program.managed_agent_sandboxes",
    workUnitRef: command.workUnitRef,
    attachmentRef: command.attachmentRef,
    attachmentGeneration,
    resourceGeneration: 1,
    version: 0,
    lastEventSequence: 0,
    target: command.target,
    imageDigest: command.imageDigest,
    profileRef: command.profileRef,
    lease: command.lease,
    budget: command.budget,
    capabilities: command.requestedCapabilities,
    facts: {
      lifecycle: "provisioning",
      leaseState: command.lease.state,
      guestState: "starting",
      filesystemState: "unallocated",
      ingressState: "closed",
      runtimeState: "none",
      acceptingWork: false,
      cleanupComplete: false,
    },
    createdAt: command.requestedAt,
    updatedAt: command.requestedAt,
  });

const materializeRuntimeEvents = (
  resource: ManagedSandboxResource,
  events: ReadonlyArray<ManagedSandboxRuntimeEventInput>,
): Effect.Effect<ReadonlyArray<ManagedSandboxEvent>, BoxV1FacadeError> =>
  Effect.forEach(events, (event, offset) =>
    digest(`${event.turnRef}\n${event.turnEventSequence}`).pipe(
      Effect.flatMap((value) =>
        decode(ManagedSandboxEventSchema, {
          ...event,
          schema: "openagents.managed_sandbox_event.v1",
          eventRef: `event.sbx.runtime.${value.slice(0, 32)}`,
          sandboxRef: resource.sandboxRef,
          sequence: resource.lastEventSequence + offset + 1,
        }),
      ),
    ),
  );

const lifecycleEvent = (
  resource: ManagedSandboxResource,
  outcome: BoxV1LifecycleOutcome,
  offset: number,
  event: Readonly<Record<string, unknown>>,
): Effect.Effect<ManagedSandboxEvent, BoxV1FacadeError> =>
  digest(`${outcome.operationRef}\n${String(event._tag)}\n${offset}`).pipe(
    Effect.flatMap((value) =>
      decode(ManagedSandboxEventSchema, {
        ...event,
        schema: "openagents.managed_sandbox_event.v1",
        eventRef: `event.sbx.lifecycle.${value.slice(0, 32)}`,
        sandboxRef: resource.sandboxRef,
        resourceGeneration: outcome.generation,
        sequence: resource.lastEventSequence + offset + 1,
        observedAt: outcome.observedAt,
      }),
    ),
  );

const materializeLifecycleEvents = (
  resource: ManagedSandboxResource,
  outcome: BoxV1LifecycleOutcome,
): Effect.Effect<ReadonlyArray<ManagedSandboxEvent>, BoxV1FacadeError> =>
  Effect.gen(function* () {
    const reasonRef = `reason.${outcome.errorCode ?? "provider_operation_failed"}`;
    let inputs: ReadonlyArray<Readonly<Record<string, unknown>>>;
    switch (outcome.phase) {
      case "ready":
        inputs = [{ _tag: "GuestReady" }];
        break;
      case "stopped": {
        const checkpoint = yield* digest(outcome.receiptRef);
        inputs = [
          { _tag: "FilesystemCheckpointed", checkpointDigest: `sha256:${checkpoint}` },
          { _tag: "GuestStopped" },
        ];
        break;
      }
      case "deleted":
        inputs = [{ _tag: "CleanupObserved" }];
        break;
      case "failed":
        inputs = [
          {
            _tag: "OperationFailed",
            operationRef: outcome.operationRef,
            errorRef: reasonRef,
          },
        ];
        break;
      case "recovery_required":
        inputs = [
          {
            _tag: "OperationFailed",
            operationRef: outcome.operationRef,
            errorRef: reasonRef,
          },
          { _tag: "RecoveryMarked", reasonRef },
        ];
        break;
    }
    return yield* Effect.forEach(inputs, (event, offset) =>
      lifecycleEvent(resource, outcome, offset, event),
    );
  });

export const makeManagedSandboxBroker = (
  input: Readonly<{
    principal: BoxV1Principal;
    policy: BoxV1Policy;
    store: BoxV1NativeStore;
    runtime: BoxV1Runtime;
    now?: (() => Date) | undefined;
  }>,
): ManagedSandboxBroker => {
  const scope = {
    ownerRef: input.principal.ownerRef,
    tenantRef: input.principal.tenantRef,
  };

  const assertScope = (command: ManagedSandboxCommand): Effect.Effect<void, BoxV1FacadeError> =>
    command.ownerRef === scope.ownerRef && command.tenantRef === scope.tenantRef
      ? Effect.void
      : Effect.fail(conflict("command does not belong to the authenticated owner scope"));

  const inspect = (sandboxRef: string) => input.store.inspect({ ...scope, sandboxRef });

  const syncTurn = (
    resource: ManagedSandboxResource,
    turn: ManagedSandboxTurn,
  ): Effect.Effect<
    Readonly<{
      turn: ManagedSandboxTurn;
      receipt: ManagedSandboxTurnReceipt | null;
      events: ReadonlyArray<ManagedSandboxEvent>;
    }>,
    BoxV1FacadeError
  > =>
    Effect.gen(function* () {
      if (["settled", "failed", "interrupted"].includes(turn.status)) {
        const current = yield* input.store.inspectTurn({
          ...scope,
          sandboxRef: resource.sandboxRef,
          turnRef: turn.turnRef,
        });
        return {
          turn: current.turn,
          receipt: current.receipt ?? null,
          events: [],
        };
      }
      const providerEvents = yield* input.runtime.sync({
        principal: input.principal,
        resource,
        turn,
        afterTurnSequence: turn.lastEventSequence,
      });
      if (providerEvents.length === 0) return { turn, receipt: null, events: [] };
      const recorded = yield* input.store.recordRuntimeEvents({
        ...scope,
        sandboxRef: resource.sandboxRef,
        turnRef: turn.turnRef,
        expectedResourceGeneration: resource.resourceGeneration,
        events: providerEvents,
      });
      return {
        turn: recorded.turn,
        receipt: recorded.receipt ?? null,
        events: recorded.events,
      };
    });

  const reserve = (
    command: ManagedSandboxCommand,
    options?: Readonly<{
      attachmentGeneration?: number | undefined;
      initialResource?: ManagedSandboxResource | undefined;
    }>,
  ) =>
    Effect.gen(function* () {
      yield* assertScope(command);
      const existing = yield* input.store.reservation({
        ...scope,
        commandRef: command.commandRef,
      });
      if (existing !== undefined) {
        const normalized = { ...command, requestedAt: existing.command.requestedAt };
        if (canonicalJson(normalized) !== canonicalJson(existing.command)) {
          return yield* conflict("command ref is bound to different request bytes");
        }
        if (existing.resource.programRef !== "program.managed_agent_sandboxes") {
          return yield* conflict("resource is outside the managed-sandbox program");
        }
        if (
          command._tag === "Create" &&
          options?.attachmentGeneration !== existing.resource.attachmentGeneration
        ) {
          return yield* conflict("command ref is bound to a different attachment generation");
        }
        return existing;
      }
      if (command._tag !== "Create") {
        const resource = yield* inspect(command.sandboxRef);
        if (resource.programRef !== "program.managed_agent_sandboxes") {
          return yield* conflict("resource is outside the managed-sandbox program");
        }
        return yield* input.store.reserve({ command });
      }
      yield* assertPolicy(input.policy, {
        target: command.target,
        imageDigest: command.imageDigest,
        profileRef: command.profileRef,
        lease: command.lease,
        budget: command.budget,
        capabilities: command.requestedCapabilities,
      });
      const attachmentGeneration = options?.attachmentGeneration;
      if (
        attachmentGeneration === undefined ||
        !Number.isSafeInteger(attachmentGeneration) ||
        attachmentGeneration < 1
      ) {
        return yield* invalid("create requires an exact positive attachment generation");
      }
      if (options?.initialResource !== undefined) {
        const resource = options.initialResource;
        if (
          resource.ownerRef !== command.ownerRef ||
          resource.tenantRef !== command.tenantRef ||
          resource.programRef !== "program.managed_agent_sandboxes" ||
          resource.workUnitRef !== command.workUnitRef ||
          resource.attachmentRef !== command.attachmentRef ||
          resource.attachmentGeneration !== attachmentGeneration ||
          resource.resourceGeneration !== 1 ||
          resource.version !== 0 ||
          !sameTarget(resource.target, command.target) ||
          resource.imageDigest !== command.imageDigest ||
          resource.profileRef !== command.profileRef ||
          JSON.stringify(resource.lease) !== JSON.stringify(command.lease) ||
          JSON.stringify(resource.budget) !== JSON.stringify(command.budget) ||
          JSON.stringify(resource.capabilities) !==
            JSON.stringify(command.requestedCapabilities) ||
          resource.facts.lifecycle !== "provisioning" ||
          resource.facts.cleanupComplete
        ) {
          return yield* conflict(
            "caller-supplied initial resource does not match the admitted create command",
          );
        }
        return yield* input.store.reserve({ command, initialResource: resource });
      }
      const suffix = yield* digest(command.commandRef);
      const resource = yield* initialResource(
        command,
        `sandbox.native.${suffix.slice(0, 32)}`,
        attachmentGeneration,
      );
      return yield* input.store.reserve({ command, initialResource: resource });
    });

  const execute: ManagedSandboxBroker["execute"] = (rawCommand, options) =>
    Effect.gen(function* () {
      const requestedCommand = yield* decode(ManagedSandboxCommandSchema, rawCommand);
      const reservation = yield* reserve(requestedCommand, options);
      const command = reservation.command;
      let resource = reservation.resource;
      let receipt = yield* syntheticReceipt(reservation);
      let turn: ManagedSandboxTurn | null = null;
      let turnReceipt: ManagedSandboxTurnReceipt | null = null;
      let events: ReadonlyArray<ManagedSandboxEvent> = [];

      if (
        ["Create", "Stop", "Resume", "Delete"].includes(command._tag) &&
        reservation.status === "pending" &&
        input.runtime.lifecycle !== undefined
      ) {
        const lifecycleCommand = command as Extract<
          ManagedSandboxCommand,
          { _tag: "Create" | "Stop" | "Resume" | "Delete" }
        >;
        const providerOutcome = yield* input.runtime.lifecycle({
          principal: input.principal,
          resource,
          command: lifecycleCommand,
        });
        const providerReceiptDigest = yield* digest(providerOutcome.receiptRef);
        events = yield* materializeLifecycleEvents(resource, providerOutcome);
        const succeeded = ["ready", "stopped", "deleted"].includes(providerOutcome.phase);
        receipt = yield* input.store.settle({
          ...scope,
          sandboxRef: resource.sandboxRef,
          commandRef: command.commandRef,
          expectedResourceGeneration: resource.resourceGeneration,
          events,
          outcome: succeeded ? "succeeded" : "failed",
          artifactRefs: [
            `artifact.managed-sandbox.lifecycle-receipt.${providerReceiptDigest}`,
          ],
          ...(providerOutcome.errorCode === null
            ? {}
            : { errorCode: `reason.${providerOutcome.errorCode}` }),
          observedAt: providerOutcome.observedAt,
        });
        resource = yield* inspect(resource.sandboxRef);
      } else if (command._tag === "Dispatch") {
        const prompt = options?.prompt;
        if (prompt === undefined || prompt.trim() === "" || prompt.length > 100_000) {
          return yield* invalid("dispatch requires a non-empty bounded prompt");
        }
        const promptDigest = yield* digest(prompt);
        if (command.promptDigest !== `sha256:${promptDigest}`) {
          return yield* conflict("dispatch prompt bytes do not match promptDigest");
        }
        const inspected = yield* input.store.inspectTurn({
          ...scope,
          sandboxRef: resource.sandboxRef,
          turnRef: command.turnRef,
        });
        turn = inspected.turn;
        turnReceipt = inspected.receipt ?? null;
        if (reservation.status === "pending" && turn.status === "pending") {
          const providerEvents = yield* input.runtime.dispatch({
            principal: input.principal,
            resource,
            turn,
            prompt,
          });
          if (providerEvents[0]?._tag !== "RuntimeStarted") {
            return yield* unavailable("agent_turn_start");
          }
          events = yield* materializeRuntimeEvents(resource, providerEvents);
          const observedAt = events.at(-1)?.observedAt;
          if (observedAt === undefined) return yield* unavailable("agent_turn_start");
          receipt = yield* input.store.settle({
            ...scope,
            sandboxRef: resource.sandboxRef,
            commandRef: command.commandRef,
            expectedResourceGeneration: resource.resourceGeneration,
            events,
            outcome: "succeeded",
            observedAt,
          });
        }
        resource = yield* inspect(resource.sandboxRef);
        const synced = yield* syncTurn(resource, turn);
        turn = synced.turn;
        turnReceipt = synced.receipt;
        events = [...events, ...synced.events];
      } else if (command._tag === "Interrupt") {
        const inspected = yield* input.store.inspectTurn({
          ...scope,
          sandboxRef: resource.sandboxRef,
          turnRef: command.turnRef,
        });
        turn = inspected.turn;
        turnReceipt = inspected.receipt ?? null;
        if (reservation.status === "pending") {
          const providerEvents = yield* input.runtime.interrupt({
            principal: input.principal,
            resource,
            turn,
            reasonRef: command.reasonRef,
            idempotencyRef: command.idempotencyRef,
          });
          if (providerEvents[0]?._tag !== "RuntimeInterruptRequested") {
            return yield* unavailable("interrupt");
          }
          events = yield* materializeRuntimeEvents(resource, providerEvents);
          const observedAt = events.at(-1)?.observedAt;
          if (observedAt === undefined) return yield* unavailable("interrupt");
          receipt = yield* input.store.settle({
            ...scope,
            sandboxRef: resource.sandboxRef,
            commandRef: command.commandRef,
            expectedResourceGeneration: resource.resourceGeneration,
            events,
            outcome: "succeeded",
            observedAt,
          });
        }
        resource = yield* inspect(resource.sandboxRef);
        const current = yield* input.store.inspectTurn({
          ...scope,
          sandboxRef: resource.sandboxRef,
          turnRef: command.turnRef,
        });
        turn = current.turn;
        turnReceipt = current.receipt ?? null;
      } else if (command._tag === "Inspect") {
        const turns = yield* input.store.turns({
          ...scope,
          sandboxRef: resource.sandboxRef,
        });
        const latest = turns.at(-1);
        if (latest !== undefined) {
          const current = yield* input.store.inspectTurn({
            ...scope,
            sandboxRef: resource.sandboxRef,
            turnRef: latest.turnRef,
          });
          const synced = yield* syncTurn(resource, current.turn);
          turn = synced.turn;
          turnReceipt = synced.receipt;
          events = synced.events;
          resource = yield* inspect(resource.sandboxRef);
        }
      }

      return { command, resource, receipt, turn, turnReceipt, events };
    });

  return {
    execute,
    list: (limit = 100) => input.store.list({ ...scope, limit }),
  };
};
