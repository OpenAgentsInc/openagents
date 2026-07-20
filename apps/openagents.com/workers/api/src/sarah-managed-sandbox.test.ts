import {
  ManagedSandboxReceiptSchema,
  ManagedSandboxResourceSchema,
  type ManagedSandboxCommand,
  type ManagedSandboxResource,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import type { InferenceToolCall } from "./inference/provider-adapter";
import type { ManagedSandboxBroker } from "./managed-sandbox-broker";
import type { BoxV1Policy } from "./managed-sandbox-box-v1-routes";
import { makeSarahManagedSandboxTools } from "./sarah-managed-sandbox";

const now = "2026-07-19T16:00:00.000Z";
const expiresAt = "2026-07-19T17:00:00.000Z";
const imageDigest = `sha256:${"a".repeat(64)}`;

const policy: BoxV1Policy = {
  target: {
    targetRef: "target.gcp.managed-sandbox.us-central1",
    targetClass: "openagents_managed",
    provider: "google_cloud",
    adapterRef: "adapter.oa-codex-control.gce.v1",
    region: "us-central1",
    isolation: "gce_vm",
    dataPosture: "openagents_managed_region",
  },
  imageDigest,
  profileRef: "profile.sbx.gce.e2-small.v1",
  defaultTtlSeconds: 3_600,
  maxTtlSeconds: 86_400,
  maxActiveBoxes: 2,
  maxCostMicros: 10_000,
  maxCpuMillis: 86_400_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
};

const budget = {
  currency: "USD" as const,
  maxCostMicros: 10_000,
  maxCpuMillis: 3_600_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
  maxLifetimeSeconds: 3_600,
};

const capabilities: ManagedSandboxResource["capabilities"] = [
  {
    capabilityRef: "capability.sarah.sbx.fixture.agent_turn",
    kind: "agent_turn",
    state: "active",
    expiresAt,
  },
];

const resource = (lifecycle: ManagedSandboxResource["facts"]["lifecycle"] = "ready") =>
  S.decodeUnknownSync(ManagedSandboxResourceSchema)({
    schema: "openagents.managed_sandbox.v1",
    sandboxRef: "sandbox.native.sarah-fixture",
    ownerRef: "owner.fixture",
    tenantRef: "owner.fixture",
    programRef: "program.managed_agent_sandboxes",
    workUnitRef: "work.sarah.fixture",
    attachmentRef: "attachment.sarah.fixture",
    attachmentGeneration: 1,
    resourceGeneration: 1,
    version: 2,
    lastEventSequence: 2,
    target: policy.target,
    imageDigest,
    profileRef: policy.profileRef,
    lease: {
      leaseRef: "lease.sarah.sbx.fixture",
      state: "active",
      issuedAt: now,
      expiresAt,
      ttlSeconds: 3_600,
      renewable: true,
    },
    budget,
    capabilities,
    facts: {
      lifecycle,
      leaseState: "active",
      guestState: lifecycle === "recovery_required" ? "unknown" : "present",
      filesystemState: lifecycle === "recovery_required" ? "unknown" : "attached",
      ingressState: "closed",
      runtimeState: "none",
      acceptingWork: lifecycle === "ready",
      cleanupComplete: lifecycle === "deleted",
    },
    createdAt: now,
    updatedAt: now,
  });

const receipt = (command: ManagedSandboxCommand, state: ManagedSandboxResource) =>
  S.decodeUnknownSync(ManagedSandboxReceiptSchema)({
    schema: "openagents.managed_sandbox_receipt.v1",
    receiptRef: `receipt.sbx.test.${command._tag.toLowerCase()}`,
    commandRef: command.commandRef,
    sandboxRef: state.sandboxRef,
    ownerRef: state.ownerRef,
    tenantRef: state.tenantRef,
    resourceGeneration: state.resourceGeneration,
    version: state.version,
    outcome: "accepted",
    lifecycle: state.facts.lifecycle,
    eventRefs: [],
    artifactRefs: [],
    observedAt: now,
  });

const toolCall = (name: string): InferenceToolCall => ({
  id: `tool.${name}`,
  type: "function",
  function: { name, arguments: "{}" },
});

const common = {
  programRef: "program.managed_agent_sandboxes",
  repositoryRef: "OpenAgentsInc/openagents",
  targetRef: policy.target.targetRef,
  imageDigest,
  profileRef: policy.profileRef,
};

const exact = {
  ...common,
  sandboxRef: "sandbox.native.sarah-fixture",
  workUnitRef: "work.sarah.fixture",
  resourceGeneration: 1,
  expectedVersion: 2,
  budget,
  capabilityRefs: ["capability.sarah.sbx.fixture.agent_turn"],
  idempotencyRef: "idempotency.fixture.action",
};

const harness = (input?: {
  allow?: boolean;
  lifecycle?: ManagedSandboxResource["facts"]["lifecycle"];
  listed?: ReadonlyArray<ManagedSandboxResource>;
  resumeGeneration?: number;
}) => {
  const commands: Array<ManagedSandboxCommand> = [];
  const attachmentGenerations: Array<number | undefined> = [];
  let effects = 0;
  const broker: ManagedSandboxBroker = {
    execute: (command, options) => {
      effects += 1;
      commands.push(command);
      attachmentGenerations.push(options?.attachmentGeneration);
      const state =
        command._tag === "Create"
          ? S.decodeUnknownSync(ManagedSandboxResourceSchema)({
              ...resource("provisioning"),
              workUnitRef: command.workUnitRef,
              attachmentRef: command.attachmentRef,
              attachmentGeneration: options?.attachmentGeneration,
              target: command.target,
              imageDigest: command.imageDigest,
              profileRef: command.profileRef,
              lease: command.lease,
              budget: command.budget,
              capabilities: command.requestedCapabilities,
              version: 1,
              facts: {
                ...resource("provisioning").facts,
                lifecycle: "provisioning",
              },
            })
          : S.decodeUnknownSync(ManagedSandboxResourceSchema)({
              ...resource(
                command._tag === "Delete" ? (input?.lifecycle ?? "deleting") : input?.lifecycle,
              ),
              resourceGeneration: command._tag === "Resume" ? (input?.resumeGeneration ?? 1) : 1,
            });
      return Effect.succeed({
        command,
        resource: state,
        receipt: receipt(command, state),
        turn: null,
        turnReceipt: null,
        events: [],
      });
    },
    list: () => {
      effects += 1;
      return Effect.succeed(input?.listed ?? [resource()]);
    },
  };
  const tools = makeSarahManagedSandboxTools({
    sql: {} as never,
    ownerUserId: "owner.fixture",
    threadRef: "thread.sarah.fixture",
    turnId: "turn.sarah.fixture",
    principal: {
      actorRef: "principal.sarah",
      ownerRef: "owner.fixture",
      tenantRef: "owner.fixture",
      login: "Sarah",
      email: null,
    },
    policy,
    broker,
    runtimeAdmitted: true,
    now: () => new Date(now),
    authorizeOperation: (_sql, authorityInput) =>
      Effect.succeed(
        (() => {
          const allowed =
            input?.allow ??
            authorityInput.conditionResults?.every((result) => result.passed) ??
            false;
          return {
            allowed,
            receiptRef: `receipt.authority.${authorityInput.action}`,
            ...(allowed ? {} : { refusalReason: "condition_failed" }),
          };
        })(),
      ),
  });
  return { attachmentGenerations, commands, effects: () => effects, tools };
};

const execute = (tools: ReturnType<typeof harness>["tools"], name: string, input: unknown) => {
  const tool = tools.find((candidate) => candidate.definition.name === name);
  if (tool === undefined) throw new Error(`missing ${name}`);
  return Effect.runPromise(tool.execute(input, toolCall(name)));
};

describe("Sarah managed-sandbox broker", () => {
  test("advertises exactly eight closed tools and no generic administration", () => {
    const names = harness().tools.map((tool) => tool.definition.name);
    expect(names).toEqual([
      "managed_sandbox_create",
      "managed_sandboxes_list",
      "managed_sandbox_inspect",
      "managed_sandbox_dispatch",
      "managed_sandbox_interrupt",
      "managed_sandbox_stop",
      "managed_sandbox_resume",
      "managed_sandbox_delete",
    ]);
    expect(names.join(" ")).not.toMatch(
      /gcloud|shell|database|topology|credential|filesystem|container_admin|full_auto_start/u,
    );
  });

  test("binds create and dispatch to exact owner, target, budget, capabilities, and prompt bytes", async () => {
    const fixture = harness();
    const created = await execute(fixture.tools, "managed_sandbox_create", {
      ...common,
      workUnitRef: "work.sarah.fixture",
      attachmentRef: "attachment.sarah.fixture",
      attachmentGeneration: 7,
      ttlSeconds: 3_600,
      budget,
      capabilityKinds: ["agent_turn"],
      idempotencyRef: "idempotency.fixture.create",
    });
    expect(created.resultRefs).toEqual(
      expect.arrayContaining([
        "receipt.authority.create_managed_sandbox",
        "receipt.sbx.test.create",
      ]),
    );
    expect(JSON.parse(created.content).activity).toMatchObject([
      { order: 1, state: "succeeded" },
      { order: 2, state: "accepted" },
    ]);
    expect(JSON.parse(created.content).resource.attachmentGeneration).toBe(7);
    expect(fixture.attachmentGenerations[0]).toBe(7);

    await execute(fixture.tools, "managed_sandbox_dispatch", {
      ...exact,
      idempotencyRef: "idempotency.fixture.dispatch",
      turnRef: "turn.sarah.managed.fixture",
      capabilityRef: "capability.sarah.sbx.fixture.agent_turn",
      prompt: "Run the bounded long-running agent task.",
      runtime: {
        provider: "codex",
        modelRef: "model.codex.default",
        harnessRef: "harness.openai.codex-sdk.v1",
      },
    });
    const dispatch = fixture.commands.find((command) => command._tag === "Dispatch");
    expect(dispatch).toMatchObject({
      ownerRef: "owner.fixture",
      tenantRef: "owner.fixture",
      expectedVersion: 2,
      capabilityRef: "capability.sarah.sbx.fixture.agent_turn",
    });
    expect(dispatch?._tag === "Dispatch" ? dispatch.promptDigest : "").toMatch(
      /^sha256:[a-f0-9]{64}$/u,
    );
  });

  test("refuses authority before any target effect", async () => {
    const fixture = harness({ allow: false });
    const result = await execute(fixture.tools, "managed_sandbox_inspect", exact);
    expect(result).toMatchObject({ authorityAllowed: false, isError: true });
    expect(fixture.effects()).toBe(0);
  });

  test("receipts target substitution as an authority refusal before target effects", async () => {
    const fixture = harness();
    const result = await execute(fixture.tools, "managed_sandbox_inspect", {
      ...exact,
      targetRef: "target.gcp.substituted",
    });
    expect(result).toMatchObject({
      authorityAllowed: false,
      authorityReceiptRef: "receipt.authority.inspect_managed_sandbox",
      isError: true,
    });
    expect(fixture.effects()).toBe(0);

    const dispatch = await execute(fixture.tools, "managed_sandbox_dispatch", {
      ...exact,
      targetRef: "target.gcp.substituted",
      idempotencyRef: "idempotency.fixture.dispatch-substituted",
      turnRef: "turn.sarah.managed.substituted",
      capabilityRef: "capability.sarah.sbx.fixture.agent_turn",
      prompt: "This must never reach the target.",
      runtime: {
        provider: "codex",
        modelRef: "model.codex.default",
        harnessRef: "harness.openai.codex-sdk.v1",
      },
    });
    expect(dispatch).toMatchObject({ authorityAllowed: false, isError: true });
    expect(fixture.effects()).toBe(0);
  });

  test("routes list, inspect, interrupt, stop, resume, and delete through their closed actions", async () => {
    const fixture = harness({ resumeGeneration: 2 });
    await execute(fixture.tools, "managed_sandboxes_list", {
      ...common,
      idempotencyRef: "idempotency.fixture.list",
    });
    await execute(fixture.tools, "managed_sandbox_inspect", {
      ...exact,
      idempotencyRef: "idempotency.fixture.inspect",
    });
    await execute(fixture.tools, "managed_sandbox_interrupt", {
      ...exact,
      idempotencyRef: "idempotency.fixture.interrupt",
      turnRef: "turn.sarah.managed.fixture",
      reasonRef: "reason.owner.interrupt",
    });
    await execute(fixture.tools, "managed_sandbox_stop", {
      ...exact,
      idempotencyRef: "idempotency.fixture.stop",
      reasonRef: "reason.owner.stop",
    });
    await execute(fixture.tools, "managed_sandbox_resume", {
      ...exact,
      idempotencyRef: "idempotency.fixture.resume",
    });
    await execute(fixture.tools, "managed_sandbox_delete", {
      ...exact,
      idempotencyRef: "idempotency.fixture.delete",
      reasonRef: "reason.owner.delete",
    });
    expect(fixture.commands.map((command) => command._tag)).toEqual([
      "Inspect",
      "Interrupt",
      "Stop",
      "Resume",
      "Delete",
    ]);
  });

  test("accepts only the exact next generation returned by resume", async () => {
    const resumed = await execute(
      harness({ resumeGeneration: 2 }).tools,
      "managed_sandbox_resume",
      {
        ...exact,
        idempotencyRef: "idempotency.fixture.resume-next-generation",
      },
    );
    expect(JSON.parse(resumed.content).resource.resourceGeneration).toBe(2);

    await expect(
      execute(harness({ resumeGeneration: 3 }).tools, "managed_sandbox_resume", {
        ...exact,
        idempotencyRef: "idempotency.fixture.resume-skipped-generation",
      }),
    ).rejects.toMatchObject({ reason: "managed_sandbox_scope_mismatch" });
  });

  test("fails closed when a list dependency crosses owner scope", async () => {
    const crossOwner = S.decodeUnknownSync(ManagedSandboxResourceSchema)({
      ...resource(),
      ownerRef: "owner.other",
      tenantRef: "owner.other",
    });
    const fixture = harness({ listed: [crossOwner] });
    await expect(
      execute(fixture.tools, "managed_sandboxes_list", {
        ...common,
        idempotencyRef: "idempotency.fixture.list",
      }),
    ).rejects.toMatchObject({ reason: "managed_sandbox_scope_mismatch" });
  });

  test("never calls recovery-required deletion successful or cleanup complete", async () => {
    const fixture = harness({ lifecycle: "recovery_required" });
    const result = await execute(fixture.tools, "managed_sandbox_delete", {
      ...exact,
      reasonRef: "reason.owner.delete",
      idempotencyRef: "idempotency.fixture.delete",
    });
    expect(result.isError).toBe(true);
    expect(result.summary).toContain("requires recovery");
    expect(result.summary).not.toContain("completed");
    expect(JSON.parse(result.content).resource.facts.cleanupComplete).toBe(false);
  });
});
