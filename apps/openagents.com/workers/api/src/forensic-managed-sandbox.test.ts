import type { ForensicBudget } from "@openagentsinc/forensic-contract";
import { ManagedSandboxResourceSchema } from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  FORENSIC_MANAGED_SANDBOX_PATH,
  assertForensicBudgetBelowPrompt,
  makeForensicManagedSandbox,
  makeForensicManagedSandboxRoutes,
} from "./forensic-managed-sandbox";
import { BoxV1FacadeError } from "./managed-sandbox-box-v1-routes";
import type {
  BoxV1LifecycleOutcome,
  BoxV1Policy,
  BoxV1Runtime,
} from "./managed-sandbox-box-v1-routes";
import {
  BoxV1MemoryAuthority,
  makeBoxV1MemoryRuntime,
} from "./managed-sandbox-box-v1.test-support";
import { type ManagedSandboxBroker, makeManagedSandboxBroker } from "./managed-sandbox-broker";

const budget: ForensicBudget = {
  maxTimeSeconds: 900,
  maxTokens: 20_000,
  maxCostMicros: 10_000,
  maxConcurrency: 1,
  maxArtifactBytes: 10_000_000,
  maxNetworkBytes: 100_000_000,
};

const usage = {
  receiptRef: "receipt.forensic.budget.fixture",
  sandboxRef: "sandbox.forensic.fixture",
  resourceGeneration: 1,
  observedAt: "2026-08-01T10:00:00.000Z",
  elapsedMillis: 10_000,
  tokens: 100,
  costMicros: 20,
  sourceBytes: 10,
  artifactBytes: 30,
  networkBytes: 40,
  activeTurns: 0,
  exact: true,
};
const budgetScope = {
  sandboxRef: usage.sandboxRef,
  resourceGeneration: usage.resourceGeneration,
  now: new Date(usage.observedAt),
};
const sourceBinding = {
  runRef: "run.forensic.fixture",
  authorityRef: "authority.forensic-source.fixture",
  bundleRef: "bundle.forensic.fixture",
  coverageRef: "coverage.forensic.fixture",
  coverageDigest: `sha256:${"c".repeat(64)}`,
  sourceDigest: `sha256:${"d".repeat(64)}`,
  materializationReceiptRef: "receipt.forensic-source.fixture",
};
const principal = {
  actorRef: "agent:owner.forensic.fixture",
  ownerRef: "owner.forensic.fixture",
  tenantRef: "owner.forensic.fixture",
  login: "Forensics",
  email: null,
};
const unusedRuntime = {} as BoxV1Runtime;

const proof = (
  action: BoxV1LifecycleOutcome["action"],
  phase: BoxV1LifecycleOutcome["phase"],
): BoxV1LifecycleOutcome => ({
  operationRef: `operation.forensic.${action}`,
  receiptRef: `receipt.forensic.${action}`,
  action,
  phase,
  generation: 1,
  readinessObserved: phase === "ready",
  cleanupObserved: phase === "deleted",
  forensicDriverRef: "driver.openagents.forensic-worker.v1",
  readinessProof: {
    providerRunning: true,
    guestMarkerObserved: true,
    imageAdmitted: true,
    noExternalIp: true,
    noGuestServiceAccount: true,
    egressDefaultDeny: true,
    brokerEgressOnly: true,
    metadataEgressOnly: true,
    controlIngressOnly: true,
    metadataRestricted: true,
    linuxGuest: true,
    bubblewrapReady: true,
    forensicDriverReady: true,
  },
  cleanupProof: {
    zeroCompute: phase === "deleted",
    zeroDisk: phase === "deleted",
    zeroFirewall: phase === "deleted",
    zeroProcess: phase === "stopped" || phase === "deleted",
    zeroScratch: phase === "stopped" || phase === "deleted",
    zeroIngress: phase === "deleted",
    zeroGrants: false,
  },
  usageProof: {
    exact: true,
    tokens: 0,
    sourceBytes: 0,
    artifactBytes: 0,
    networkBytes: 0,
    activeTurns: 0,
  },
  measuredRunningMs: 1,
  measuredCostMicros: 1,
  errorCode: null,
  observedAt: "2026-08-01T10:00:00.000Z",
});

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
  imageDigest: `sha256:${"a".repeat(64)}`,
  profileRef: "profile.sbx.gce.e2-small.v1",
  defaultTtlSeconds: 900,
  maxTtlSeconds: 900,
  maxActiveBoxes: 1,
  maxCostMicros: 10_000,
  maxCpuMillis: 900_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
};

describe("forensic managed sandbox", () => {
  test("admits only when every below-prompt budget has remaining measured capacity", async () => {
    await expect(
      Effect.runPromise(assertForensicBudgetBelowPrompt(budget, usage, budgetScope)),
    ).resolves.toBeUndefined();

    for (const refused of [
      { ...usage, exact: false },
      { ...usage, activeTurns: 1 },
      { ...usage, elapsedMillis: budget.maxTimeSeconds * 1_000 },
      { ...usage, tokens: budget.maxTokens },
      { ...usage, costMicros: budget.maxCostMicros },
      { ...usage, artifactBytes: budget.maxArtifactBytes },
      { ...usage, networkBytes: budget.maxNetworkBytes },
    ]) {
      await expect(
        Effect.runPromise(assertForensicBudgetBelowPrompt(budget, refused, budgetScope)),
      ).rejects.toBeDefined();
    }
  });

  test("refuses a foreign target before invoking the broker", async () => {
    let brokerInvoked = false;
    const broker = {
      execute: () => {
        brokerInvoked = true;
        return Effect.die("broker must not run");
      },
      list: () => Effect.succeed([]),
    } as ManagedSandboxBroker;
    const service = makeForensicManagedSandbox({
      broker,
      principal,
      runtime: unusedRuntime,
      policy: {
        ...policy,
        target: { ...policy.target, provider: "box" as "google_cloud" },
      },
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
      assertSourceReady: () =>
        Effect.succeed({
          expiresAt: "2026-08-01T10:15:00.000Z",
          release: Effect.void,
        }),
    });

    await expect(
      Effect.runPromise(
        service.admit({
          commandRef: "command.forensic.create.fixture",
          idempotencyRef: "idempotency.forensic.create.fixture",
          requestedByRef: "principal.forensic.fixture",
          ownerRef: "owner.forensic.fixture",
          tenantRef: "tenant.forensic.fixture",
          workUnitRef: "work.forensic.fixture",
          attachmentRef: "attachment.forensic.fixture",
          placementRef: "placement.forensic.fixture",
          regionRef: "region.google-cloud.us-central1",
          budgetRef: "budget.forensic.fixture",
          requestedAt: "2026-08-01T10:00:00.000Z",
          expiresAt: "2026-08-01T10:15:00.000Z",
          budget,
        }),
      ),
    ).rejects.toBeDefined();
    expect(brokerInvoked).toBe(false);
  });

  test("does not expose resume, stop, checkpoint, or restore operations", () => {
    const service = makeForensicManagedSandbox({
      broker: {
        execute: () => Effect.die("unused"),
        list: () => Effect.succeed([]),
      },
      principal,
      runtime: unusedRuntime,
      policy,
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
      assertSourceReady: () =>
        Effect.succeed({
          expiresAt: "2026-08-01T10:15:00.000Z",
          release: Effect.void,
        }),
    });
    expect(Object.keys(service).sort()).toEqual([
      "admit",
      "cancel",
      "collectArtifact",
      "delete",
      "dispatch",
    ]);
  });

  test("binds private I/O and dispatch to one durable generation, then revokes, stops, and deletes", async () => {
    const authority = new BoxV1MemoryAuthority();
    const memoryRuntime = makeBoxV1MemoryRuntime();
    const lifecycleActions: Array<BoxV1LifecycleOutcome["action"]> = [];
    let probeCalls = 0;
    let dispatchDeadlineAt: string | undefined;
    const runtime: BoxV1Runtime = {
      ...memoryRuntime,
      dispatch: (input) => {
        dispatchDeadlineAt = input.guardrails?.deadlineAt;
        return memoryRuntime.dispatch(input);
      },
      lifecycle: (input) => {
        const action = input.command._tag.toLowerCase() as "create" | "stop" | "resume" | "delete";
        lifecycleActions.push(action);
        const phase =
          action === "create" || action === "resume"
            ? "ready"
            : action === "stop"
              ? "stopped"
              : "deleted";
        return Effect.succeed({
          ...proof(action, phase),
          operationRef: input.command.commandRef,
          generation: input.resource.resourceGeneration,
        });
      },
      probe: (input) => {
        probeCalls += 1;
        return Effect.succeed({
          ...proof("probe", "ready"),
          operationRef: input.operationRef,
          generation: input.resource.resourceGeneration,
        });
      },
      artifact: (input) =>
        Effect.succeed({
          bytes: new TextEncoder().encode("artifact"),
          contentType: "application/zstd",
          receipt: {
            schemaVersion: "openagents.managed_sandbox_guest_io_receipt.v1",
            receiptRef: "receipt.forensic.artifact.fixture",
            operationRef: input.operationRef,
            sandboxRef: input.resource.sandboxRef,
            resourceGeneration: input.resource.resourceGeneration,
            capabilityRef: input.capabilityRef,
            action: "read_artifact",
            outcome: "succeeded",
            pathDigest: `sha256:${"c".repeat(64)}`,
            startedAt: input.requestedAt,
            finishedAt: input.requestedAt,
            bytesRead: 8,
            bytesWritten: 0,
            cpuMillis: 1,
            networkBytes: 0,
            processTerminated: true,
            descendantsRemaining: 0,
            scratchCleaned: true,
            ingressClosed: true,
            egressDenied: true,
            pathPolicy: "resolved_beneath_workspace_root",
            symlinkTraversal: false,
            secretScan: "clean",
            evidenceRefs: ["evidence.forensic.artifact.fixture"],
          },
          artifact: {
            schemaVersion: "openagents.managed_sandbox_artifact_receipt.v1",
            artifactRef: "artifact.forensic.fixture",
            contentDigest:
              "sha256:c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c",
            byteLength: 8,
            sourceGeneration: input.resource.resourceGeneration,
            sourcePathDigest: `sha256:${"c".repeat(64)}`,
            retentionUntil: input.retentionUntil,
            contentType: "application/zstd",
            evidenceRefs: ["evidence.forensic.artifact.fixture"],
          },
        }),
    };
    const broker = makeManagedSandboxBroker({
      principal,
      policy,
      store: authority,
      runtime,
    });
    const service = makeForensicManagedSandbox({
      broker,
      policy,
      principal,
      runtime,
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
      assertSourceReady: () =>
        Effect.succeed({
          expiresAt: "2026-08-01T10:05:00.000Z",
          release: Effect.void,
        }),
      now: () => new Date("2026-08-01T10:00:00.000Z"),
    });
    const placement = await Effect.runPromise(
      service.admit({
        commandRef: "command.forensic.create.fixture",
        idempotencyRef: "idempotency.forensic.create.fixture",
        requestedByRef: principal.actorRef,
        ownerRef: principal.ownerRef,
        tenantRef: principal.tenantRef,
        workUnitRef: "work.forensic.fixture",
        attachmentRef: "attachment.forensic.fixture",
        placementRef: "placement.forensic.fixture",
        regionRef: "region.google-cloud.us-central1",
        budgetRef: "budget.forensic.fixture",
        requestedAt: "2026-08-01T10:00:00.000Z",
        expiresAt: "2026-08-01T10:15:00.000Z",
        budget,
      }),
    );
    await expect(
      Effect.runPromise(
        service.dispatch(
          { ...placement, profileDigest: `sha256:${"e".repeat(64)}` },
          {
            commandRef: "command.forensic.dispatch.forged",
            idempotencyRef: "idempotency.forensic.dispatch.forged",
            requestedByRef: principal.actorRef,
            turnRef: "turn.forensic.forged",
            capabilityRef: placement.capabilityRefs[0]!,
            requestedAt: "2026-08-01T10:00:00.000Z",
            prompt: "must not run",
            sourceBinding,
          },
        ),
      ),
    ).rejects.toBeDefined();
    await expect(
      Effect.runPromise(
        service.cancel(
          { ...placement, leaseRef: "lease.forensic.forged" },
          {
            commandRef: "command.forensic.cancel.forged",
            inspectCommandRef: "command.forensic.inspect.forged",
            idempotencyRef: "idempotency.forensic.cancel.forged",
            inspectIdempotencyRef: "idempotency.forensic.inspect.forged",
            requestedByRef: principal.actorRef,
            turnRef: "turn.forensic.forged",
            reasonRef: "reason.forensic.forged",
            requestedAt: "2026-08-01T10:00:00.000Z",
          },
        ),
      ),
    ).rejects.toBeDefined();
    for (const [suffix, capabilityRef] of [
      ["source", placement.capabilityRefs[1]!],
      ["artifact", placement.capabilityRefs[2]!],
      ["unknown", "capability.forensic.unknown"],
    ] as const) {
      await expect(
        Effect.runPromise(
          service.dispatch(placement, {
            commandRef: `command.forensic.dispatch.${suffix}`,
            idempotencyRef: `idempotency.forensic.dispatch.${suffix}`,
            requestedByRef: principal.actorRef,
            turnRef: `turn.forensic.${suffix}`,
            capabilityRef,
            requestedAt: "2026-08-01T10:00:00.000Z",
            prompt: "must not run",
            sourceBinding,
          }),
        ),
      ).rejects.toBeDefined();
    }
    await expect(
      Effect.runPromise(
        service.dispatch(
          {
            ...placement,
            capabilityRefs: ["capability.forensic.forged", ...placement.capabilityRefs.slice(1)],
          },
          {
            commandRef: "command.forensic.dispatch.forged-capability",
            idempotencyRef: "idempotency.forensic.dispatch.forged-capability",
            requestedByRef: principal.actorRef,
            turnRef: "turn.forensic.forged-capability",
            capabilityRef: "capability.forensic.forged",
            requestedAt: "2026-08-01T10:00:00.000Z",
            prompt: "must not run",
            sourceBinding,
          },
        ),
      ),
    ).rejects.toBeDefined();
    expect(probeCalls).toBe(0);
    await expect(
      Effect.runPromise(
        service.delete(
          { ...placement, capabilityRefs: placement.capabilityRefs.slice(1) },
          {
            commandRef: "command.forensic.delete.forged",
            idempotencyRef: "idempotency.forensic.delete.forged",
            requestedByRef: principal.actorRef,
            reasonRef: "reason.forensic.forged",
            requestedAt: "2026-08-01T10:00:00.000Z",
          },
        ),
      ),
    ).rejects.toBeDefined();
    await expect(
      Effect.runPromise(
        service.collectArtifact(
          {
            ...placement,
            resourceGeneration: placement.resourceGeneration + 1,
          },
          "2026-08-01T10:00:00.000Z",
        ),
      ),
    ).rejects.toBeDefined();
    await expect(
      Effect.runPromise(service.collectArtifact(placement, "2026-08-01T10:00:00.000Z")),
    ).resolves.toMatchObject({
      encoding: "base64",
      sandboxRef: placement.sandboxRef,
      resourceGeneration: placement.resourceGeneration,
      capabilityRef: placement.capabilityRefs[2],
    });
    await Effect.runPromise(
      service.dispatch(placement, {
        commandRef: "command.forensic.dispatch.fixture",
        idempotencyRef: "idempotency.forensic.dispatch.fixture",
        requestedByRef: principal.actorRef,
        turnRef: "turn.forensic.fixture",
        capabilityRef: placement.capabilityRefs[0]!,
        requestedAt: "2026-08-01T10:00:00.000Z",
        prompt: "inspect the admitted source",
        sourceBinding,
      }),
    );
    expect(dispatchDeadlineAt).toBe("2026-08-01T10:05:00.000Z");
    await Effect.runPromise(
      service.cancel(placement, {
        commandRef: "command.forensic.cancel.fixture",
        inspectCommandRef: "command.forensic.inspect.fixture",
        idempotencyRef: "idempotency.forensic.cancel.fixture",
        inspectIdempotencyRef: "idempotency.forensic.inspect.fixture",
        requestedByRef: principal.actorRef,
        turnRef: "turn.forensic.fixture",
        reasonRef: "reason.forensic.owner_cancelled",
        requestedAt: "2026-08-01T10:00:00.000Z",
      }),
    );
    const cleaned = await Effect.runPromise(
      service.delete(placement, {
        commandRef: "command.forensic.cleanup.fixture",
        idempotencyRef: "idempotency.forensic.cleanup.fixture",
        requestedByRef: principal.actorRef,
        reasonRef: "reason.forensic.complete",
        requestedAt: "2026-08-01T10:00:00.000Z",
      }),
    );
    expect(cleaned).toMatchObject({
      state: "cleaned",
      stopReceiptRef: expect.any(String),
    });
    const durable = authority.resources.get(placement.sandboxRef);
    expect(durable?.facts).toMatchObject({
      lifecycle: "deleted",
      cleanupComplete: true,
    });
    expect(durable?.capabilities.every((capability) => capability.state === "revoked")).toBe(true);
    expect(lifecycleActions).toEqual(["create", "stop", "delete"]);
    expect(probeCalls).toBe(1);
  });

  test("refuses a stale or cross-generation budget receipt before prompt dispatch", async () => {
    await expect(
      Effect.runPromise(
        assertForensicBudgetBelowPrompt(
          budget,
          { ...usage, observedAt: "2026-08-01T09:59:00.000Z" },
          budgetScope,
        ),
      ),
    ).rejects.toBeDefined();
    await expect(
      Effect.runPromise(
        assertForensicBudgetBelowPrompt(budget, { ...usage, resourceGeneration: 2 }, budgetScope),
      ),
    ).rejects.toBeDefined();
    await expect(
      Effect.runPromise(
        assertForensicBudgetBelowPrompt(
          budget,
          { ...usage, observedAt: "not-a-timestamp" },
          budgetScope,
        ),
      ),
    ).rejects.toBeDefined();
  });

  test("durably reconciles probe recovery and deletes directly from recovery-required", async () => {
    const authority = new BoxV1MemoryAuthority();
    const memoryRuntime = makeBoxV1MemoryRuntime();
    const lifecycleActions: Array<BoxV1LifecycleOutcome["action"]> = [];
    const runtime: BoxV1Runtime = {
      ...memoryRuntime,
      lifecycle: (input) => {
        const action = input.command._tag.toLowerCase() as "create" | "stop" | "resume" | "delete";
        lifecycleActions.push(action);
        return Effect.succeed({
          ...proof(action, action === "delete" ? "deleted" : "ready"),
          operationRef: input.command.commandRef,
          generation: input.resource.resourceGeneration,
        });
      },
      probe: (input) => {
        const { usageProof: _usageProof, ...recoveryProof } = proof("probe", "recovery_required");
        return Effect.succeed({
          ...recoveryProof,
          operationRef: input.operationRef,
          generation: input.resource.resourceGeneration,
          readinessObserved: false,
          errorCode: "forensic_usage_unavailable",
        });
      },
    };
    const broker = makeManagedSandboxBroker({
      principal,
      policy,
      store: authority,
      runtime,
    });
    const service = makeForensicManagedSandbox({
      broker,
      policy,
      principal,
      runtime,
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
      assertSourceReady: () =>
        Effect.succeed({
          expiresAt: "2026-08-01T10:15:00.000Z",
          release: Effect.void,
        }),
      now: () => new Date("2026-08-01T10:00:00.000Z"),
    });
    const placement = await Effect.runPromise(
      service.admit({
        commandRef: "command.forensic.recovery.create",
        idempotencyRef: "idempotency.forensic.recovery.create",
        requestedByRef: principal.actorRef,
        ownerRef: principal.ownerRef,
        tenantRef: principal.tenantRef,
        workUnitRef: "work.forensic.recovery",
        attachmentRef: "attachment.forensic.recovery",
        placementRef: "placement.forensic.recovery",
        regionRef: "region.google-cloud.us-central1",
        budgetRef: "budget.forensic.recovery",
        requestedAt: "2026-08-01T10:00:00.000Z",
        expiresAt: "2026-08-01T10:15:00.000Z",
        budget,
      }),
    );
    await expect(
      Effect.runPromise(
        service.dispatch(placement, {
          commandRef: "command.forensic.recovery.dispatch",
          idempotencyRef: "idempotency.forensic.recovery.dispatch",
          requestedByRef: principal.actorRef,
          turnRef: "turn.forensic.recovery",
          capabilityRef: placement.capabilityRefs[0]!,
          requestedAt: "2026-08-01T10:00:00.000Z",
          prompt: "must not run after a failed probe",
          sourceBinding,
        }),
      ),
    ).rejects.toBeDefined();
    expect(authority.resources.get(placement.sandboxRef)?.facts.lifecycle).toBe(
      "recovery_required",
    );
    expect(
      authority.eventsBySandbox
        .get(placement.sandboxRef)
        ?.some((event) => event._tag === "RecoveryMarked"),
    ).toBe(true);
    await expect(
      Effect.runPromise(
        service.delete(placement, {
          commandRef: "command.forensic.recovery.cleanup",
          idempotencyRef: "idempotency.forensic.recovery.cleanup",
          requestedByRef: principal.actorRef,
          reasonRef: "reason.forensic.recovery",
          requestedAt: "2026-08-01T10:00:01.000Z",
        }),
      ),
    ).resolves.toMatchObject({ state: "cleaned" });
    expect(authority.resources.get(placement.sandboxRef)?.facts.lifecycle).toBe("deleted");
    expect(lifecycleActions).toEqual(["create", "delete"]);
  });

  test("refuses before runtime/model dispatch when durable source authority is absent", async () => {
    let runtimeDispatches = 0;
    const sourcePrincipal = {
      actorRef: "agent:owner.forensic.source-gate",
      ownerRef: "owner.forensic.source-gate",
      tenantRef: "tenant.forensic.source-gate",
      login: "Forensics",
      email: null,
    };
    const resource = S.decodeUnknownSync(ManagedSandboxResourceSchema)({
      schema: "openagents.managed_sandbox.v1",
      sandboxRef: "sandbox.forensic.source-gate",
      ownerRef: "owner.forensic.source-gate",
      tenantRef: "tenant.forensic.source-gate",
      programRef: "program.managed_agent_sandboxes",
      workUnitRef: "work.forensic.source-gate",
      attachmentRef: "attachment.forensic.source-gate",
      attachmentGeneration: 1,
      resourceGeneration: 1,
      version: 1,
      lastEventSequence: 1,
      target: policy.target,
      imageDigest: policy.imageDigest,
      profileRef: policy.profileRef,
      lease: {
        leaseRef: "lease.forensic.source-gate",
        state: "active",
        issuedAt: "2026-08-01T10:00:00.000Z",
        expiresAt: "2026-08-01T10:15:00.000Z",
        ttlSeconds: 900,
        renewable: false,
      },
      budget: {
        currency: "USD",
        maxCostMicros: budget.maxCostMicros,
        maxCpuMillis: budget.maxTimeSeconds * 1_000,
        maxNetworkBytes: budget.maxNetworkBytes,
        maxArtifactBytes: budget.maxArtifactBytes,
        maxLifetimeSeconds: budget.maxTimeSeconds,
      },
      capabilities: [
        {
          capabilityRef: "capability.work.forensic.source-gate.agent_turn",
          kind: "agent_turn",
          state: "active",
          expiresAt: "2026-08-01T10:15:00.000Z",
        },
      ],
      facts: {
        lifecycle: "ready",
        leaseState: "active",
        guestState: "present",
        filesystemState: "attached",
        ingressState: "broker_only",
        runtimeState: "none",
        acceptingWork: true,
        cleanupComplete: false,
      },
      createdAt: "2026-08-01T10:00:00.000Z",
      updatedAt: "2026-08-01T10:00:01.000Z",
    });
    const broker = {
      list: () => Effect.succeed([resource]),
      execute: () => {
        runtimeDispatches += 1;
        return Effect.die("broker dispatch must not run without source authority");
      },
    } as ManagedSandboxBroker;
    const service = makeForensicManagedSandbox({
      broker,
      policy,
      principal: sourcePrincipal,
      runtime: {
        ...unusedRuntime,
        probe: (input) =>
          Effect.succeed({
            ...proof("probe", "ready"),
            operationRef: input.operationRef,
            generation: input.resource.resourceGeneration,
          }),
      },
      profileDigest: `sha256:${"b".repeat(64)}`,
      resolveBudget: () => Effect.succeed(budget),
      assertSourceReady: () =>
        Effect.fail(
          new BoxV1FacadeError({
            code: "conflict",
            status: 409,
            message: "durable source authority is absent",
            retryable: false,
          }),
        ),
    });
    const placement = {
      schema: "openagents.forensic_worker_placement.v1" as const,
      placementRef: "placement.forensic.source-gate",
      ownerRef: resource.ownerRef,
      tenantRef: resource.tenantRef,
      workUnitRef: resource.workUnitRef,
      sandboxRef: resource.sandboxRef,
      attachmentGeneration: resource.attachmentGeneration,
      resourceGeneration: resource.resourceGeneration,
      targetClass: "openagents_managed" as const,
      provider: "google_cloud" as const,
      adapterRef: "adapter.oa-codex-control.gce.v1" as const,
      isolation: "gce_vm" as const,
      regionRef: "region.google-cloud.us-central1",
      imageDigest: policy.imageDigest,
      profileDigest: `sha256:${"b".repeat(64)}` as const,
      networkPolicyRef: "network-policy-ref://openagents/managed-sandbox/broker-only-v1" as const,
      leaseRef: resource.lease.leaseRef,
      budgetRef: "budget.forensic.source-gate",
      capabilityRefs: ["capability.work.forensic.source-gate.agent_turn"],
      state: "worker_ready" as const,
      admissionReceiptRef: "receipt.forensic.source-gate.admission",
      readinessReceiptRef: "receipt.forensic.source-gate.readiness",
      updatedAt: "2026-08-01T10:00:01.000Z",
    };

    await expect(
      Effect.runPromise(
        service.dispatch(placement, {
          commandRef: "command.forensic.source-gate.dispatch",
          idempotencyRef: "idempotency.forensic.source-gate.dispatch",
          requestedByRef: sourcePrincipal.actorRef,
          turnRef: "turn.forensic.source-gate",
          capabilityRef: `capability.work.forensic.source-gate.agent_turn`,
          requestedAt: "2026-08-01T10:01:00.000Z",
          prompt: "Inspect the pinned source.",
          sourceBinding: {
            runRef: "run.forensic.source-gate",
            authorityRef: "authority.forensic-source.absent",
            bundleRef: "bundle.forensic.source-gate",
            coverageRef: "coverage.forensic.source-gate",
            coverageDigest: `sha256:${"c".repeat(64)}`,
            sourceDigest: `sha256:${"d".repeat(64)}`,
            materializationReceiptRef: "receipt.forensic-source.absent",
          },
        }),
      ),
    ).rejects.toBeDefined();
    expect(runtimeDispatches).toBe(0);
  });

  test("the live route is authenticated, default-off, and exposes one exact path", async () => {
    const routes = makeForensicManagedSandboxRoutes({
      authenticateOwner: async () => ({ userId: "owner.forensic.fixture" }),
      enabled: () => false,
      policy: () => Effect.die("policy must not run while disabled"),
      profileDigest: () => undefined,
      store: () => {
        throw new Error("store must not run while disabled");
      },
      runtime: () => Effect.die("runtime must not run while disabled"),
      assertSourceReady: () => Effect.die("authority must not run while disabled"),
    });
    const response = await Effect.runPromise(
      routes.handle(
        new Request(`https://api.openagents.com${FORENSIC_MANAGED_SANDBOX_PATH}`, {
          method: "POST",
          body: JSON.stringify({ _tag: "Admit" }),
        }),
        {},
        {} as ExecutionContext,
      ),
    );
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toMatchObject({
      error: "runtime_not_admitted",
    });
  });
});
