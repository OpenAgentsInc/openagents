import {
  ManagedSandboxResourceSchema,
  type ManagedSandboxCommand,
  type ManagedSandboxResource,
  type ManagedSandboxTurn,
} from "@openagentsinc/managed-sandbox-contract";
import { Context, Effect, Exit, Layer, Ref, Scope } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdeAgentCodeSnapshotSchema,
  emptyIdeAgentCodeSnapshot,
  type IdeAgentCodeSnapshot,
} from "./agent-code-contract.ts";
import { ideAgentFixtureAttachment, ideAgentFixtureDigest } from "./agent-code-fixture.ts";
import { IdeAttachmentGenerationSchema } from "./project-contract.ts";
import {
  IdeManagedSandboxCommandSchema,
  IdeManagedSandboxGatewayResultSchema,
  type IdeManagedSandboxAdmission,
  type IdeManagedSandboxGatewayResult,
} from "./managed-sandbox-contract.ts";
import {
  IdeManagedSandboxService,
  makeIdeManagedSandboxLayer,
  type IdeManagedSandboxGateway,
} from "./managed-sandbox-service.ts";
import { openIdeManagedSandboxHost } from "./managed-sandbox-host.ts";
import type { IdeAgentCodeHost } from "./agent-code-host.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

const now = "2026-07-19T20:00:00.000Z";
const later = "2026-07-19T20:30:00.000Z";
const target = {
  targetRef: "target.gcp.desktop",
  targetClass: "openagents_managed" as const,
  provider: "google_cloud" as const,
  adapterRef: "adapter.gcp.desktop",
  region: "us-central1",
  isolation: "gce_vm" as const,
  dataPosture: "openagents_managed_region" as const,
};

const admission: IdeManagedSandboxAdmission = {
  _tag: "Available",
  target,
  imageDigest: ideAgentFixtureDigest("a"),
  profileRef: "profile.desktop.codex",
  lease: {
    leaseRef: "lease.desktop.fixture",
    state: "active",
    issuedAt: now,
    expiresAt: later,
    ttlSeconds: 1_800,
    renewable: true,
  },
  budget: {
    currency: "USD",
    maxCostMicros: 2_000_000,
    maxCpuMillis: 600_000,
    maxNetworkBytes: 0,
    maxArtifactBytes: 16 * 1024 * 1024,
    maxLifetimeSeconds: 1_800,
  },
  requestedCapabilities: [
    {
      capabilityRef: "capability.desktop.agent-turn",
      kind: "agent_turn",
      state: "active",
      expiresAt: later,
    },
  ],
  networkPosture: "deny_all",
  custody: "openagents_managed_region",
  retentionRef: "retention.desktop.fixture",
  checkedAt: now,
};

const agentSnapshot = (attachment = ideAgentFixtureAttachment()): IdeAgentCodeSnapshot =>
  IdeAgentCodeSnapshotSchema.make({
    ...emptyIdeAgentCodeSnapshot(),
    lifecycle: "attached",
    attachment,
    revision: 1,
  });

const mutationAuthority = (
  attachment = ideAgentFixtureAttachment(),
  current: () => boolean = () => true,
): IdePortableMutationAuthority => {
  const permit: IdePortableMutationPermit = {
    _tag: "LocalOnly",
    key: `local:${attachment.grantRef}:${attachment.sessionRef}:work-context.desktop.fixture`,
    grantRef: attachment.grantRef,
    sessionRef: attachment.sessionRef,
    workContextRef: "work-context.desktop.fixture",
    attachmentRef: null,
    generation: null,
    targetRef: null,
  };
  return {
    authorize: (grantRef) =>
      current() && grantRef === permit.grantRef
        ? { _tag: "Permitted", permit }
        : { _tag: "Refused", reason: "admission_unavailable" },
    reauthorize: (candidate) => current() && candidate.key === permit.key,
  };
};

const factsFor = (lifecycle: ManagedSandboxResource["facts"]["lifecycle"]) => ({
  lifecycle,
  leaseState: lifecycle === "deleted" ? ("released" as const) : ("active" as const),
  guestState:
    lifecycle === "stopped" || lifecycle === "deleted" ? ("absent" as const) : ("present" as const),
  filesystemState:
    lifecycle === "stopped"
      ? ("durable" as const)
      : lifecycle === "deleted"
        ? ("deleted" as const)
        : ("attached" as const),
  ingressState:
    lifecycle === "deleted"
      ? ("revoked" as const)
      : lifecycle === "stopped"
        ? ("closed" as const)
        : ("broker_only" as const),
  runtimeState: lifecycle === "running" ? ("running" as const) : ("none" as const),
  acceptingWork: ["ready", "idle", "running"].includes(lifecycle),
  cleanupComplete: lifecycle === "deleted",
});

const makeResource = (
  input: Readonly<{
    version: number;
    generation: number;
    lifecycle: ManagedSandboxResource["facts"]["lifecycle"];
    attachmentGeneration?: number;
    targetRef?: string;
  }>,
): ManagedSandboxResource =>
  ManagedSandboxResourceSchema.make({
    schema: "openagents.managed_sandbox.v1",
    sandboxRef: "sandbox.desktop.fixture",
    ownerRef: "owner.desktop.fixture",
    tenantRef: "tenant.desktop.fixture",
    programRef: "program.managed_agent_sandboxes",
    workUnitRef: "work-unit.desktop.fixture",
    attachmentRef: "ide.agent-attachment.fixture",
    attachmentGeneration: input.attachmentGeneration ?? 1,
    resourceGeneration: input.generation,
    version: input.version,
    lastEventSequence: input.version,
    target: { ...target, targetRef: input.targetRef ?? "target.gcp.desktop" },
    imageDigest: ideAgentFixtureDigest("a"),
    profileRef: "profile.desktop.codex",
    lease:
      admission._tag === "Available"
        ? admission.lease
        : {
            leaseRef: "lease.desktop.fixture",
            state: "active",
            issuedAt: now,
            expiresAt: later,
            ttlSeconds: 1_800,
            renewable: true,
          },
    budget:
      admission._tag === "Available"
        ? admission.budget
        : {
            currency: "USD",
            maxCostMicros: 0,
            maxCpuMillis: 0,
            maxNetworkBytes: 0,
            maxArtifactBytes: 0,
            maxLifetimeSeconds: 1,
          },
    capabilities: admission._tag === "Available" ? admission.requestedCapabilities : [],
    facts: factsFor(input.lifecycle),
    createdAt: now,
    updatedAt: now,
  });

const fakeGateway = (
  options: Readonly<{
    targetRef?: string;
    attachmentGeneration?: number;
    inspectAdvancesResource?: boolean;
  }> = {},
): Readonly<{
  gateway: IdeManagedSandboxGateway;
  commands: ReadonlyArray<ManagedSandboxCommand>;
  executions: ReadonlyArray<Readonly<{ prompt?: string; attachmentGeneration?: number }>>;
}> => {
  const commands: Array<ManagedSandboxCommand> = [];
  const executions: Array<Readonly<{ prompt?: string; attachmentGeneration?: number }>> = [];
  let version = 0;
  let generation = 0;
  let lifecycle: ManagedSandboxResource["facts"]["lifecycle"] = "ready";
  let turn: ManagedSandboxTurn | null = null;
  const execute = (
    command: ManagedSandboxCommand,
    executionOptions: Readonly<{
      prompt?: string;
      attachmentGeneration?: number;
      signal?: AbortSignal;
    }> = {},
  ): IdeManagedSandboxGatewayResult => {
    commands.push(command);
    executions.push({
      ...(executionOptions.prompt === undefined ? {} : { prompt: executionOptions.prompt }),
      ...(executionOptions.attachmentGeneration === undefined
        ? {}
        : { attachmentGeneration: executionOptions.attachmentGeneration }),
    });
    let receiptVersion: number | undefined;
    switch (command._tag) {
      case "Create":
        version = 1;
        lifecycle = "ready";
        break;
      case "Inspect":
        if (options.inspectAdvancesResource) {
          receiptVersion = version;
          version += 1;
          if (turn !== null) turn = { ...turn, lastEventSequence: version };
        }
        break;
      case "Dispatch":
        version += 1;
        lifecycle = "running";
        turn = {
          schema: "openagents.managed_sandbox_turn.v1",
          turnRef: command.turnRef,
          sandboxRef: command.sandboxRef,
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          workUnitRef: "work-unit.desktop.fixture",
          attachmentRef: "ide.agent-attachment.fixture",
          attachmentGeneration: 1,
          resourceGeneration: generation,
          turnSequence: 1,
          lastEventSequence: 1,
          commandRef: command.commandRef,
          capabilityRef: command.capabilityRef,
          promptDigest: command.promptDigest,
          runtime: command.runtime,
          status: "running",
          createdAt: now,
          startedAt: now,
        };
        break;
      case "Interrupt":
        version += 1;
        lifecycle = "idle";
        if (turn !== null) turn = { ...turn, status: "interrupted", settledAt: now };
        break;
      case "Stop":
        version += 1;
        lifecycle = "stopped";
        turn = null;
        break;
      case "Resume":
        version += 1;
        generation += 1;
        lifecycle = "ready";
        break;
      case "Delete":
        version += 1;
        lifecycle = "deleted";
        turn = null;
        break;
      case "Update":
        version += 1;
        break;
    }
    const resource = makeResource({
      version,
      generation,
      lifecycle,
      targetRef: options.targetRef,
      attachmentGeneration: options.attachmentGeneration ?? executionOptions.attachmentGeneration,
    });
    return IdeManagedSandboxGatewayResultSchema.make({
      command,
      resource,
      receipt: {
        schema: "openagents.managed_sandbox_receipt.v1",
        receiptRef: `receipt.desktop.${command.commandRef}`,
        commandRef: command.commandRef,
        sandboxRef: resource.sandboxRef,
        ownerRef: resource.ownerRef,
        tenantRef: resource.tenantRef,
        resourceGeneration: resource.resourceGeneration,
        version: receiptVersion ?? resource.version,
        outcome: "succeeded",
        lifecycle: resource.facts.lifecycle,
        eventRefs: [],
        artifactRefs: [],
        observedAt: now,
      },
      turn,
      turnReceipt: null,
      events:
        receiptVersion === undefined
          ? []
          : [
              {
                schema: "openagents.managed_sandbox_event.v1",
                _tag: "RuntimeTextDelta",
                eventRef: `event.desktop.inspect.${resource.version}`,
                sandboxRef: resource.sandboxRef,
                resourceGeneration: resource.resourceGeneration,
                sequence: resource.lastEventSequence,
                observedAt: now,
                turnRef: turn?.turnRef ?? "turn.desktop.fixture",
                turnEventSequence: 2,
                content: "still running",
              },
            ],
    });
  };
  return {
    commands,
    executions,
    gateway: {
      admission: () => Effect.succeed(admission),
      execute: (command, executionOptions) => Effect.sync(() => execute(command, executionOptions)),
    },
  };
};

const request = (sequence: number, attachment = ideAgentFixtureAttachment()) => ({
  requestRef: `command.desktop.${sequence}`,
  idempotencyRef: `idempotency.desktop.${sequence}`,
  requestedAt: now,
  expectedAttachment: attachment,
});

describe("IDE managed sandbox", () => {
  test("keeps one project and agent identity across create, turn, interrupt, stop, resume, and delete", async () => {
    const fixture = fakeGateway();
    const attachment = ideAgentFixtureAttachment();
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission",
            idempotencyRef: "idempotency.desktop.admission",
            requestedAt: now,
          }),
        );
        const created = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Create",
            ...request(1, attachment),
            workUnitRef: "work-unit.desktop.fixture",
          }),
        );
        const dispatched = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Dispatch",
            ...request(2, attachment),
            sandboxRef: "sandbox.desktop.fixture",
            turnRef: "turn.desktop.fixture",
            capabilityRef: "capability.desktop.agent-turn",
            prompt: "Inspect the attached project and report the next bounded change.",
            promptDigest: ideAgentFixtureDigest("c"),
            runtime: {
              provider: "codex",
              modelRef: "model.gpt-5",
              harnessRef: "harness.codex",
            },
          }),
        );
        const interrupted = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Interrupt",
            ...request(3, attachment),
            sandboxRef: "sandbox.desktop.fixture",
            turnRef: "turn.desktop.fixture",
            reasonRef: "reason.owner.interrupt",
          }),
        );
        const stopped = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Stop",
            ...request(4, attachment),
            sandboxRef: "sandbox.desktop.fixture",
            reasonRef: "reason.owner.stop",
          }),
        );
        const resumed = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Resume",
            ...request(5, attachment),
            sandboxRef: "sandbox.desktop.fixture",
          }),
        );
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Stop",
            ...request(6, attachment),
            sandboxRef: "sandbox.desktop.fixture",
            reasonRef: "reason.owner.stop-again",
          }),
        );
        const deleted = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Delete",
            ...request(7, attachment),
            sandboxRef: "sandbox.desktop.fixture",
            reasonRef: "reason.owner.delete",
          }),
        );
        return { created, dispatched, interrupted, stopped, resumed, deleted };
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway: fixture.gateway,
            mutationAuthority: mutationAuthority(attachment),
            currentAgentSnapshot: () => Effect.succeed(agentSnapshot(attachment)),
          }),
        ),
      ),
    );

    const bindings = Object.values(result).map((snapshot) => snapshot.binding);
    expect(bindings.every((binding) => binding?.projectRef === attachment.projectRef)).toBe(true);
    expect(bindings.every((binding) => binding?.worktreeRef === attachment.worktreeRef)).toBe(true);
    expect(bindings.every((binding) => binding?.sessionRef === attachment.sessionRef)).toBe(true);
    expect(
      bindings.every((binding) => binding?.agentAttachmentRef === attachment.agentAttachmentRef),
    ).toBe(true);
    expect(result.dispatched.turn?.status).toBe("running");
    expect(result.interrupted.turn?.status).toBe("interrupted");
    expect(result.stopped.projectCapability?.state._tag).toBe("Stopped");
    expect(result.resumed.resource?.resourceGeneration).toBe(1);
    expect(result.deleted.resource?.facts.cleanupComplete).toBe(true);
    expect(result.deleted.projectCapability?.state._tag).toBe("Stopped");
    expect(fixture.commands.map((command) => command._tag)).toEqual([
      "Create",
      "Dispatch",
      "Interrupt",
      "Stop",
      "Resume",
      "Stop",
      "Delete",
    ]);
    expect(fixture.executions[0]).toEqual({ attachmentGeneration: 1 });
    expect(fixture.executions[1]).toEqual({
      prompt: "Inspect the attached project and report the next bounded change.",
    });
  });

  test("accepts an inspect receipt that precedes one evidenced runtime projection", async () => {
    const fixture = fakeGateway({ inspectAdvancesResource: true });
    const attachment = ideAgentFixtureAttachment();
    const inspected = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission-inspect",
            idempotencyRef: "idempotency.desktop.admission-inspect",
            requestedAt: now,
          }),
        );
        const created = yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Create",
            ...request(10, attachment),
            workUnitRef: "work-unit.desktop.fixture",
          }),
        );
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Dispatch",
            ...request(11, attachment),
            sandboxRef: created.resource!.sandboxRef,
            turnRef: "turn.desktop.inspect",
            capabilityRef: "capability.desktop.agent-turn",
            prompt: "Keep the runtime active for inspection.",
            promptDigest: ideAgentFixtureDigest("d"),
            runtime: {
              provider: "codex",
              modelRef: "model.gpt-5",
              harnessRef: "harness.codex",
            },
          }),
        );
        return yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Inspect",
            ...request(12, attachment),
            sandboxRef: created.resource!.sandboxRef,
          }),
        );
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway: fixture.gateway,
            mutationAuthority: mutationAuthority(attachment),
            currentAgentSnapshot: () => Effect.succeed(agentSnapshot(attachment)),
          }),
        ),
      ),
    );

    expect(inspected.resource?.version).toBe(3);
    expect(inspected.receipts.at(-1)?.version).toBe(2);
    expect(inspected.events.at(-1)?.sequence).toBe(inspected.resource?.lastEventSequence);
  });

  test("rejects an attachment generation fork before sending a mutation", async () => {
    const fixture = fakeGateway();
    const original = ideAgentFixtureAttachment();
    const current = IdeAgentCodeSnapshotSchema.make({
      ...agentSnapshot(original),
      attachment: {
        ...original,
        attachmentGeneration: IdeAttachmentGenerationSchema.make(2),
      },
      revision: 2,
    });
    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission-stale",
            idempotencyRef: "idempotency.desktop.admission-stale",
            requestedAt: now,
          }),
        );
        return yield* service
          .command(
            IdeManagedSandboxCommandSchema.make({
              _tag: "Create",
              ...request(20, original),
              workUnitRef: "work-unit.desktop.fixture",
            }),
          )
          .pipe(Effect.flip);
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway: fixture.gateway,
            mutationAuthority: mutationAuthority(original),
            currentAgentSnapshot: () => Effect.succeed(current),
          }),
        ),
      ),
    );

    expect(failure.reason).toBe("stale_attachment");
    expect(fixture.commands).toHaveLength(0);
  });

  test("refuses missing portable attachment authority before gateway dispatch", async () => {
    const fixture = fakeGateway();
    const attachment = ideAgentFixtureAttachment();
    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission-no-portable-authority",
            idempotencyRef: "idempotency.desktop.admission-no-portable-authority",
            requestedAt: now,
          }),
        );
        return yield* service
          .command(
            IdeManagedSandboxCommandSchema.make({
              _tag: "Create",
              ...request(21, attachment),
              workUnitRef: "work-unit.desktop.fixture",
            }),
          )
          .pipe(Effect.flip);
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway: fixture.gateway,
            mutationAuthority: mutationAuthority(attachment, () => false),
            currentAgentSnapshot: () => Effect.succeed(agentSnapshot(attachment)),
          }),
        ),
      ),
    );

    expect(failure.reason).toBe("capability_denied");
    expect(fixture.commands).toHaveLength(0);
  });

  test("suppresses a late gateway result after portable attachment revocation", async () => {
    const fixture = fakeGateway();
    const attachment = ideAgentFixtureAttachment();
    let active = true;
    let releaseGateway: () => void = () => undefined;
    let markStarted: (() => void) | null = null;
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const blocked = new Promise<void>((resolve) => {
      releaseGateway = resolve;
    });
    const gateway: IdeManagedSandboxGateway = {
      admission: fixture.gateway.admission,
      execute: (command, options) =>
        Effect.tryPromise({
          try: async () => {
            markStarted?.();
            await blocked;
            return await Effect.runPromise(fixture.gateway.execute(command, options));
          },
          catch: () => new Error("blocked fixture failed"),
        }),
    };
    const resultPromise = Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission-revoked",
            idempotencyRef: "idempotency.desktop.admission-revoked",
            requestedAt: now,
          }),
        );
        const result = yield* service
          .command(
            IdeManagedSandboxCommandSchema.make({
              _tag: "Create",
              ...request(22, attachment),
              workUnitRef: "work-unit.desktop.fixture",
            }),
          )
          .pipe(
            Effect.match({
              onFailure: (error) => ({ _tag: "Failed" as const, error }),
              onSuccess: (snapshot) => ({ _tag: "Succeeded" as const, snapshot }),
            }),
          );
        return { result, snapshot: yield* service.snapshot() };
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway,
            mutationAuthority: mutationAuthority(attachment, () => active),
            currentAgentSnapshot: () => Effect.succeed(agentSnapshot(attachment)),
          }),
        ),
      ),
    );

    await started;
    active = false;
    releaseGateway();
    const settled = await resultPromise;

    expect(settled.result).toMatchObject({
      _tag: "Failed",
      error: { reason: "capability_denied" },
    });
    expect(fixture.commands).toHaveLength(1);
    expect(settled.snapshot.revision).toBe(1);
    expect(settled.snapshot.resource).toBeNull();
    expect(settled.snapshot.receipts).toEqual([]);
  });

  test("quiesce waits for an aborted gateway operation to unwind and is idempotent", async () => {
    const fixture = fakeGateway();
    const attachment = ideAgentFixtureAttachment();
    let releaseGateway: () => void = () => undefined;
    let markStarted: (() => void) | null = null;
    let markAborted: (() => void) | null = null;
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const aborted = new Promise<void>((resolve) => {
      markAborted = resolve;
    });
    const blocked = new Promise<void>((resolve) => {
      releaseGateway = resolve;
    });
    const gateway: IdeManagedSandboxGateway = {
      admission: fixture.gateway.admission,
      execute: (command, options) =>
        Effect.tryPromise({
          try: async () => {
            markStarted?.();
            options?.signal?.addEventListener("abort", () => markAborted?.(), { once: true });
            await blocked;
            return await Effect.runPromise(fixture.gateway.execute(command, options));
          },
          catch: () => new Error("blocked fixture failed"),
        }),
    };
    const scope = await Effect.runPromise(Scope.make());
    const context = await Effect.runPromise(
      Layer.buildWithScope(
        makeIdeManagedSandboxLayer({
          principal: {
            ownerRef: "owner.desktop.fixture",
            tenantRef: "tenant.desktop.fixture",
            requestedByRef: "principal.desktop.fixture",
          },
          gateway,
          mutationAuthority: mutationAuthority(attachment),
          currentAgentSnapshot: () => Effect.succeed(agentSnapshot(attachment)),
        }),
        scope,
      ),
    );
    const service = Context.get(context, IdeManagedSandboxService);
    await Effect.runPromise(
      service.command(
        IdeManagedSandboxCommandSchema.make({
          _tag: "RefreshAdmission",
          requestRef: "command.desktop.admission-quiesce",
          idempotencyRef: "idempotency.desktop.admission-quiesce",
          requestedAt: now,
        }),
      ),
    );
    const commandPromise = Effect.runPromise(
      service
        .command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "Create",
            ...request(23, attachment),
            workUnitRef: "work-unit.desktop.fixture",
          }),
        )
        .pipe(
          Effect.match({
            onFailure: (error) => ({ _tag: "Failed" as const, error }),
            onSuccess: (snapshot) => ({ _tag: "Succeeded" as const, snapshot }),
          }),
        ),
    );

    await started;
    let quiesced = false;
    const firstQuiesce = Effect.runPromise(service.quiesce()).then(() => {
      quiesced = true;
    });
    const secondQuiesce = Effect.runPromise(service.quiesce());
    await aborted;
    expect(quiesced).toBe(false);
    releaseGateway();
    await Promise.all([firstQuiesce, secondQuiesce]);
    const result = await commandPromise;
    const snapshot = await Effect.runPromise(service.snapshot());
    await Effect.runPromise(service.quiesce());
    await Effect.runPromise(Scope.close(scope, Exit.void));

    expect(result).toMatchObject({ _tag: "Failed", error: { reason: "gateway_unavailable" } });
    expect(snapshot.revision).toBe(1);
    expect(snapshot.resource).toBeNull();
    expect(snapshot.receipts).toEqual([]);
  });

  test("rejects managed-target substitution and private renderer material", async () => {
    const fixture = fakeGateway({ targetRef: "target.gcp.substituted" });
    const attachmentRef = yieldAgentRef();
    const stateRef = await Effect.runPromise(Ref.make(agentSnapshot(attachmentRef)));
    const failure = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* IdeManagedSandboxService;
        yield* service.command(
          IdeManagedSandboxCommandSchema.make({
            _tag: "RefreshAdmission",
            requestRef: "command.desktop.admission-substitution",
            idempotencyRef: "idempotency.desktop.admission-substitution",
            requestedAt: now,
          }),
        );
        return yield* service
          .command(
            IdeManagedSandboxCommandSchema.make({
              _tag: "Create",
              ...request(30, attachmentRef),
              workUnitRef: "work-unit.desktop.fixture",
            }),
          )
          .pipe(Effect.flip);
      }).pipe(
        Effect.provide(
          makeIdeManagedSandboxLayer({
            principal: {
              ownerRef: "owner.desktop.fixture",
              tenantRef: "tenant.desktop.fixture",
              requestedByRef: "principal.desktop.fixture",
            },
            gateway: fixture.gateway,
            mutationAuthority: mutationAuthority(attachmentRef),
            currentAgentSnapshot: () => Ref.get(stateRef),
          }),
        ),
      ),
    );

    expect(failure.reason).toBe("invalid_response");
    const publicBytes = JSON.stringify(await Effect.runPromise(Ref.get(stateRef)));
    expect(publicBytes).not.toMatch(/bearer|accessToken|refreshToken|\/Users\//u);
  });

  test("keeps the Desktop bearer in the main-owned HTTP header", async () => {
    const calls: Array<Readonly<{ authorization: string | null; body: string }>> = [];
    const fetchImpl: typeof fetch = async (_input, init) => {
      const headers = new Headers(init?.headers);
      calls.push({
        authorization: headers.get("authorization"),
        body: typeof init?.body === "string" ? init.body : "",
      });
      return new Response(JSON.stringify({ admission }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    };
    const agentCodeHost: IdeAgentCodeHost = {
      workspaceGrantRef: "grant.desktop.fixture",
      snapshot: async () => agentSnapshot(),
      command: async () => ({
        _tag: "Refused",
        reason: "unavailable",
        message: "not used",
        snapshot: agentSnapshot(),
      }),
      dispose: async () => undefined,
    };
    const host = await openIdeManagedSandboxHost({
      enabled: true,
      credential: () => ({
        ownerUserId: "owner.desktop.fixture",
        accessToken: "desktop-access-secret",
        refreshToken: "desktop-refresh-secret",
      }),
      baseUrl: "https://openagents.test/",
      agentCodeHost,
      mutationAuthority: mutationAuthority(),
      fetchImpl,
      now: () => new Date(now),
    });
    const result = await host.command(
      IdeManagedSandboxCommandSchema.make({
        _tag: "RefreshAdmission",
        requestRef: "command.desktop.http-admission",
        idempotencyRef: "idempotency.desktop.http-admission",
        requestedAt: now,
      }),
    );
    const firstDispose = host.dispose();
    const secondDispose = host.dispose();
    expect(secondDispose).toBe(firstDispose);
    await Promise.all([firstDispose, secondDispose]);

    expect(result._tag).toBe("Succeeded");
    expect(calls).toHaveLength(1);
    expect(calls[0]?.authorization).toBe("Bearer desktop-access-secret");
    expect(calls[0]?.body).not.toMatch(/desktop-access-secret|desktop-refresh-secret/u);
    expect(JSON.stringify(result)).not.toMatch(/desktop-access-secret|desktop-refresh-secret/u);
  });

  test("does not persist a gateway response that arrives after authority revocation", async () => {
    const fixture = fakeGateway();
    const attachment = ideAgentFixtureAttachment();
    const directory = mkdtempSync(path.join(tmpdir(), "openagents-managed-sandbox-"));
    const persistencePath = path.join(directory, "snapshot.json");
    let active = true;
    let releaseGateway: () => void = () => undefined;
    let markStarted: (() => void) | null = null;
    const started = new Promise<void>((resolve) => {
      markStarted = resolve;
    });
    const blocked = new Promise<void>((resolve) => {
      releaseGateway = resolve;
    });
    const gateway: IdeManagedSandboxGateway = {
      admission: fixture.gateway.admission,
      execute: (command, options) =>
        Effect.tryPromise({
          try: async () => {
            markStarted?.();
            await blocked;
            return await Effect.runPromise(fixture.gateway.execute(command, options));
          },
          catch: () => new Error("blocked fixture failed"),
        }),
    };
    const agentCodeHost: IdeAgentCodeHost = {
      workspaceGrantRef: attachment.grantRef,
      snapshot: async () => agentSnapshot(attachment),
      command: async () => ({
        _tag: "Refused",
        reason: "unavailable",
        message: "not used",
        snapshot: agentSnapshot(attachment),
      }),
      dispose: async () => undefined,
    };
    const host = await openIdeManagedSandboxHost({
      enabled: true,
      credential: () => ({
        ownerUserId: "owner.desktop.fixture",
        accessToken: "desktop-access-secret",
        refreshToken: "desktop-refresh-secret",
      }),
      baseUrl: "https://openagents.test",
      agentCodeHost,
      mutationAuthority: mutationAuthority(attachment, () => active),
      persistencePath,
      gateway,
      now: () => new Date(now),
    });
    await host.command(
      IdeManagedSandboxCommandSchema.make({
        _tag: "RefreshAdmission",
        requestRef: "command.desktop.persistence-admission",
        idempotencyRef: "idempotency.desktop.persistence-admission",
        requestedAt: now,
      }),
    );
    const baseline = readFileSync(persistencePath, "utf8");
    const pending = host.command(
      IdeManagedSandboxCommandSchema.make({
        _tag: "Create",
        ...request(24, attachment),
        workUnitRef: "work-unit.desktop.fixture",
      }),
    );

    await started;
    active = false;
    releaseGateway();
    const result = await pending;
    const persisted = readFileSync(persistencePath, "utf8");
    await host.dispose();
    rmSync(directory, { recursive: true, force: true });

    expect(result).toMatchObject({ _tag: "Refused", reason: "capability_denied" });
    expect(persisted).toBe(baseline);
  });

  test("is default-off without opening a network path", async () => {
    let calls = 0;
    const host = await openIdeManagedSandboxHost({
      enabled: false,
      credential: () => null,
      baseUrl: "https://openagents.test",
      agentCodeHost: {
        workspaceGrantRef: "grant.desktop.fixture",
        snapshot: async () => agentSnapshot(),
        command: async () => ({
          _tag: "Refused",
          reason: "unavailable",
          message: "not used",
          snapshot: agentSnapshot(),
        }),
        dispose: async () => undefined,
      },
      mutationAuthority: mutationAuthority(),
      fetchImpl: async () => {
        calls += 1;
        return new Response("{}", { status: 500 });
      },
      now: () => new Date(now),
    });
    const result = await host.command(
      IdeManagedSandboxCommandSchema.make({
        _tag: "RefreshAdmission",
        requestRef: "command.desktop.default-off",
        idempotencyRef: "idempotency.desktop.default-off",
        requestedAt: now,
      }),
    );

    expect(result).toMatchObject({ _tag: "Refused", reason: "not_configured" });
    expect(calls).toBe(0);
  });
});

const yieldAgentRef = () => ideAgentFixtureAttachment();
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
