import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { runMigrations } from "./migrate.js";
import { ManagedSandboxStoreError, PostgresManagedSandboxStore } from "./managed-sandbox-store.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const ownerRef = "owner.sbx01";
const tenantRef = "tenant.sbx01";
const observed = (offset: number): string =>
  new Date(Date.UTC(2026, 6, 19, 16, offset, 0)).toISOString();
const sha = (value: string): `sha256:${string}` => `sha256:${value.repeat(64)}`;
const runtime = {
  provider: "codex" as const,
  modelRef: "model.gpt-5.6",
  harnessRef: "harness.codex.app-server.v1",
  reasoningEffort: "high",
};

const target = {
  targetRef: "target.sbx01.gcp",
  targetClass: "openagents_managed" as const,
  provider: "google_cloud" as const,
  adapterRef: "adapter.sbx01.gce.v1",
  region: "us-central1",
  isolation: "gce_vm" as const,
  dataPosture: "openagents_managed_region" as const,
};

const budget = {
  currency: "USD" as const,
  maxCostMicros: 1_000_000,
  maxCpuMillis: 1_000_000,
  maxNetworkBytes: 10_000_000,
  maxArtifactBytes: 1_000_000,
  maxLifetimeSeconds: 3_600,
};

const lease = (suffix: string, expiresOffset = 60) => ({
  leaseRef: `lease.sbx01.${suffix}`,
  state: "active" as const,
  issuedAt: observed(0),
  expiresAt: observed(expiresOffset),
  ttlSeconds: expiresOffset * 60,
  renewable: true,
});

const createCommand = (suffix: string) => ({
  _tag: "Create" as const,
  schema: "openagents.managed_sandbox_command.v1" as const,
  commandRef: `command.sbx01.${suffix}.create`,
  requestedByRef: "principal.sol.sbx01",
  ownerRef,
  tenantRef,
  idempotencyRef: `idem.sbx01.${suffix}.create`,
  requestedAt: observed(1),
  workUnitRef: `work.sbx01.${suffix}`,
  attachmentRef: `attachment.sbx01.${suffix}`,
  target,
  imageDigest: sha("a"),
  profileRef: "profile.sbx01.gce.cpu.v1",
  lease: lease(suffix),
  budget,
  requestedCapabilities: [
    {
      capabilityRef: `capability.sbx01.${suffix}.turn`,
      kind: "agent_turn" as const,
      state: "active" as const,
      expiresAt: observed(60),
    },
  ],
});

const initialResource = (suffix: string, command = createCommand(suffix)) => ({
  schema: "openagents.managed_sandbox.v1" as const,
  sandboxRef: `sandbox.sbx01.${suffix}`,
  ownerRef,
  tenantRef,
  programRef: "program.managed_agent_sandboxes" as const,
  workUnitRef: command.workUnitRef,
  attachmentRef: command.attachmentRef,
  attachmentGeneration: 1,
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
    lifecycle: "provisioning" as const,
    leaseState: "active" as const,
    guestState: "starting" as const,
    filesystemState: "unallocated" as const,
    ingressState: "closed" as const,
    runtimeState: "none" as const,
    acceptingWork: false,
    cleanupComplete: false,
  },
  createdAt: observed(1),
  updatedAt: observed(1),
});

const event = (
  suffix: string,
  kind: string,
  generation: number,
  sequence: number,
  extra: Readonly<Record<string, unknown>> = {},
) => ({
  _tag: kind,
  schema: "openagents.managed_sandbox_event.v1",
  eventRef: `event.sbx01.${suffix}.${sequence}.${kind.toLowerCase()}`,
  sandboxRef: `sandbox.sbx01.${suffix}`,
  resourceGeneration: generation,
  sequence,
  observedAt: observed(sequence + 1),
  ...extra,
});

const commandBase = (suffix: string, kind: string, version: number) => ({
  _tag: kind,
  schema: "openagents.managed_sandbox_command.v1" as const,
  commandRef: `command.sbx01.${suffix}.${kind.toLowerCase()}.${version}`,
  requestedByRef: "principal.sol.sbx01",
  ownerRef,
  tenantRef,
  idempotencyRef: `idem.sbx01.${suffix}.${kind.toLowerCase()}.${version}`,
  requestedAt: observed(version + 10),
  sandboxRef: `sandbox.sbx01.${suffix}`,
  expectedVersion: version,
});

describe.skipIf(!hasLocalPostgres())("SBX-01 managed sandbox Postgres authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let store: PostgresManagedSandboxStore;

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe("CREATE DATABASE khala_sync_managed_sandbox");
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_managed_sandbox") });
    sql = SQL({ url: pg.urlFor("khala_sync_managed_sandbox"), max: 20 });
    store = new PostgresManagedSandboxStore(sql as unknown as SyncSql);
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  const ready = async (suffix: string): Promise<void> => {
    const command = createCommand(suffix);
    const reserved = await store.reserve({
      command,
      initialResource: initialResource(suffix, command),
    });
    expect(reserved.resource).toMatchObject({ version: 1, lastEventSequence: 1 });
    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: command.commandRef,
      expectedResourceGeneration: 1,
      events: [event(suffix, "GuestReady", 1, 2)],
      outcome: "succeeded",
      observedAt: observed(3),
    });
  };

  const stop = async (
    suffix: string,
    version: number,
    generation: number,
    startSequence: number,
  ): Promise<void> => {
    const command = {
      ...commandBase(suffix, "Stop", version),
      _tag: "Stop" as const,
      reasonRef: `reason.sbx01.${suffix}.stop`,
    };
    await store.reserve({ command });
    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: command.commandRef,
      expectedResourceGeneration: generation,
      events: [
        event(suffix, "FilesystemCheckpointed", generation, startSequence + 1, {
          checkpointDigest: sha("b"),
        }),
        event(suffix, "GuestStopped", generation, startSequence + 2),
      ],
      outcome: "succeeded",
      observedAt: observed(startSequence + 3),
    });
  };

  test("binds create bytes before effects and refuses idempotency or concurrent-create conflicts", async () => {
    const suffix = "idempotency";
    const command = createCommand(suffix);
    const resource = initialResource(suffix, command);
    const first = await store.reserve({ command, initialResource: resource });
    expect(first).toMatchObject({ disposition: "reserved", status: "pending" });

    const replay = await store.reserve({ command, initialResource: resource });
    expect(replay).toMatchObject({ disposition: "replayed", status: "pending" });
    expect(replay.resource).toEqual(first.resource);

    const durableReplay = await store.reservation({
      ownerRef,
      tenantRef,
      commandRef: command.commandRef,
    });
    expect(durableReplay).toEqual(replay);
    await expect(
      store.reservation({
        ownerRef: "owner.sbx01.other",
        tenantRef,
        commandRef: command.commandRef,
      }),
    ).rejects.toMatchObject({ code: "permission_denied" });

    const conflictingBytes = {
      ...command,
      commandRef: `${command.commandRef}.other`,
      profileRef: "profile.sbx01.gce.memory.v1",
    };
    await expect(
      store.reserve({
        command: conflictingBytes,
        initialResource: {
          ...resource,
          profileRef: conflictingBytes.profileRef,
        },
      }),
    ).rejects.toMatchObject({ code: "idempotency_conflict" });

    const secondCreate = {
      ...command,
      commandRef: `${command.commandRef}.parallel`,
      idempotencyRef: `${command.idempotencyRef}.parallel`,
    };
    await expect(
      store.reserve({ command: secondCreate, initialResource: resource }),
    ).rejects.toMatchObject({ code: "command_conflict" });
  });

  test("serializes stop, lost acknowledgements, resume generation fencing, and stale callers", async () => {
    const suffix = "lifecycle";
    await ready(suffix);

    const stopA = {
      ...commandBase(suffix, "Stop", 2),
      _tag: "Stop" as const,
      reasonRef: "reason.sbx01.lifecycle.stop-a",
    };
    const stopB = {
      ...stopA,
      commandRef: "command.sbx01.lifecycle.stop-b",
      idempotencyRef: "idem.sbx01.lifecycle.stop-b",
    };
    const concurrent = await Promise.allSettled([
      store.reserve({ command: stopA }),
      store.reserve({ command: stopB }),
    ]);
    expect(concurrent.filter((result) => result.status === "fulfilled")).toHaveLength(1);
    expect(concurrent.filter((result) => result.status === "rejected")).toHaveLength(1);
    const winning = concurrent.find((result) => result.status === "fulfilled");
    if (winning?.status !== "fulfilled") throw new Error("expected one stop winner");
    const winningCommand = winning.value.command;

    const restarted = new PostgresManagedSandboxStore(sql as unknown as SyncSql);
    const pending = await restarted.pending({ ownerRef, tenantRef });
    expect(pending.some((item) => item.command.commandRef === winningCommand.commandRef)).toBe(
      true,
    );
    const settleInput = {
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: winningCommand.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "FilesystemCheckpointed", 1, 4, { checkpointDigest: sha("c") }),
        event(suffix, "GuestStopped", 1, 5),
      ],
      outcome: "succeeded" as const,
      observedAt: observed(6),
    };
    const settled = await restarted.settle(settleInput);
    expect(await restarted.settle(settleInput)).toEqual(settled);
    await expect(
      restarted.settle({ ...settleInput, observedAt: observed(7) }),
    ).rejects.toMatchObject({ code: "event_conflict" });

    const resume = {
      ...commandBase(suffix, "Resume", 4),
      _tag: "Resume" as const,
    };
    await restarted.reserve({ command: resume });
    await restarted.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: resume.commandRef,
      expectedResourceGeneration: 1,
      events: [event(suffix, "GuestReady", 2, 7)],
      outcome: "succeeded",
      observedAt: observed(8),
    });
    const resumed = await restarted.inspect({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
    });
    expect(resumed).toMatchObject({ resourceGeneration: 2, version: 6 });
    expect(resumed.facts).toMatchObject({ lifecycle: "ready", acceptingWork: true });

    const generationRows: ReadonlyArray<{
      resource_generation: string | number;
      accepting_work: boolean;
    }> = await sql`
      SELECT resource_generation, accepting_work
      FROM khala_sync_managed_sandbox_generations
      WHERE sandbox_ref = ${`sandbox.sbx01.${suffix}`}
      ORDER BY resource_generation
    `;
    expect(
      generationRows.map((row) => [Number(row.resource_generation), row.accepting_work]),
    ).toEqual([
      [1, false],
      [2, true],
    ]);

    const staleStop = {
      ...commandBase(suffix, "Stop", 4),
      _tag: "Stop" as const,
      commandRef: "command.sbx01.lifecycle.stop-stale",
      idempotencyRef: "idem.sbx01.lifecycle.stop-stale",
      reasonRef: "reason.sbx01.lifecycle.stop-stale",
    };
    await expect(restarted.reserve({ command: staleStop })).rejects.toMatchObject({
      code: "stale_version",
    });

    const currentStop = {
      ...commandBase(suffix, "Stop", 6),
      _tag: "Stop" as const,
      reasonRef: "reason.sbx01.lifecycle.stop-current",
    };
    await restarted.reserve({ command: currentStop });
    const currentStopSettlement = {
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: currentStop.commandRef,
      events: [
        event(suffix, "FilesystemCheckpointed", 2, 9, { checkpointDigest: sha("f") }),
        event(suffix, "GuestStopped", 2, 10),
      ],
      outcome: "succeeded" as const,
      observedAt: observed(11),
    };
    await expect(
      restarted.settle({
        ...currentStopSettlement,
        expectedResourceGeneration: 1,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    await restarted.settle({ ...currentStopSettlement, expectedResourceGeneration: 2 });
  });

  test("keeps uncertain cleanup recovery-required and never fabricates deletion", async () => {
    const suffix = "recovery";
    await ready(suffix);
    await stop(suffix, 2, 1, 3);
    const deletion = {
      ...commandBase(suffix, "Delete", 4),
      _tag: "Delete" as const,
      reasonRef: "reason.sbx01.recovery.delete",
    };
    await store.reserve({ command: deletion });
    const receipt = await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: deletion.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "OperationFailed", 1, 7, {
          operationRef: "operation.sbx01.recovery.cleanup",
          errorRef: "error.sbx01.recovery.cleanup",
        }),
        event(suffix, "RecoveryMarked", 1, 8, {
          reasonRef: "reason.sbx01.recovery.uncertain-cleanup",
        }),
      ],
      outcome: "failed",
      errorCode: "cleanup.uncertain",
      observedAt: observed(9),
    });
    expect(receipt).toMatchObject({ outcome: "failed", lifecycle: "recovery_required" });
    const resource = await store.inspect({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
    });
    expect(resource.facts).toMatchObject({
      lifecycle: "recovery_required",
      acceptingWork: false,
      cleanupComplete: false,
    });
  });

  test("rolls back event gaps and records successful cleanup only after observation", async () => {
    const suffix = "cleanup";
    await ready(suffix);
    const stopCommand = {
      ...commandBase(suffix, "Stop", 2),
      _tag: "Stop" as const,
      reasonRef: "reason.sbx01.cleanup.stop",
    };
    await store.reserve({ command: stopCommand });
    await expect(
      store.settle({
        ownerRef,
        tenantRef,
        sandboxRef: `sandbox.sbx01.${suffix}`,
        commandRef: stopCommand.commandRef,
        expectedResourceGeneration: 1,
        events: [event(suffix, "FilesystemCheckpointed", 1, 5, { checkpointDigest: sha("9") })],
        outcome: "succeeded",
        observedAt: observed(6),
      }),
    ).rejects.toMatchObject({ code: "invalid_transition" });
    expect(
      await store.inspect({ ownerRef, tenantRef, sandboxRef: `sandbox.sbx01.${suffix}` }),
    ).toMatchObject({ version: 3, lastEventSequence: 3, facts: { lifecycle: "stopping" } });

    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: stopCommand.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "FilesystemCheckpointed", 1, 4, { checkpointDigest: sha("8") }),
        event(suffix, "GuestStopped", 1, 5),
      ],
      outcome: "succeeded",
      observedAt: observed(6),
    });
    const deletion = {
      ...commandBase(suffix, "Delete", 4),
      _tag: "Delete" as const,
      reasonRef: "reason.sbx01.cleanup.delete",
    };
    await store.reserve({ command: deletion });
    const receipt = await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: deletion.commandRef,
      expectedResourceGeneration: 1,
      events: [event(suffix, "CleanupObserved", 1, 7)],
      outcome: "succeeded",
      artifactRefs: ["artifact.sbx01.cleanup.zero-residue"],
      observedAt: observed(8),
    });
    expect(receipt).toMatchObject({ lifecycle: "deleted", outcome: "succeeded" });
    expect(
      await store.inspect({ ownerRef, tenantRef, sandboxRef: `sandbox.sbx01.${suffix}` }),
    ).toMatchObject({
      facts: {
        lifecycle: "deleted",
        cleanupComplete: true,
        acceptingWork: false,
        guestState: "absent",
      },
    });
  });

  test("orders turns, reconnects native events, and fences compatibility cursors", async () => {
    const suffix = "events";
    await ready(suffix);
    const firstDispatch = {
      ...commandBase(suffix, "Dispatch", 2),
      _tag: "Dispatch" as const,
      turnRef: "turn.sbx01.events.1",
      capabilityRef: "capability.sbx01.events.turn",
      promptDigest: sha("d"),
      runtime,
    };
    expect(await store.reserve({ command: firstDispatch })).toMatchObject({ turnSequence: 1 });

    const competing = {
      ...firstDispatch,
      commandRef: "command.sbx01.events.dispatch.competing",
      idempotencyRef: "idem.sbx01.events.dispatch.competing",
      turnRef: "turn.sbx01.events.competing",
    };
    await expect(store.reserve({ command: competing })).rejects.toMatchObject({
      code: "command_conflict",
    });

    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: firstDispatch.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "RuntimeStarted", 1, 3, {
          turnRef: firstDispatch.turnRef,
          turnEventSequence: 1,
        }),
        event(suffix, "RuntimeSettled", 1, 4, {
          turnRef: firstDispatch.turnRef,
          turnEventSequence: 2,
          finishReason: "structural_completion",
        }),
      ],
      outcome: "succeeded",
      observedAt: observed(5),
    });
    const secondDispatch = {
      ...commandBase(suffix, "Dispatch", 4),
      _tag: "Dispatch" as const,
      turnRef: "turn.sbx01.events.2",
      capabilityRef: "capability.sbx01.events.turn",
      promptDigest: sha("e"),
      runtime: { ...runtime, provider: "claude", modelRef: "model.claude.sonnet" },
    };
    expect(await store.reserve({ command: secondDispatch })).toMatchObject({ turnSequence: 2 });
    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      commandRef: secondDispatch.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "RuntimeStarted", 1, 5, {
          turnRef: secondDispatch.turnRef,
          turnEventSequence: 1,
        }),
        event(suffix, "RuntimeSettled", 1, 6, {
          turnRef: secondDispatch.turnRef,
          turnEventSequence: 2,
          finishReason: "structural_completion",
        }),
      ],
      outcome: "succeeded",
      observedAt: observed(7),
    });
    expect(
      await store.turns({ ownerRef, tenantRef, sandboxRef: `sandbox.sbx01.${suffix}` }),
    ).toEqual([
      { turnSequence: 1, turnRef: firstDispatch.turnRef, status: "settled" },
      { turnSequence: 2, turnRef: secondDispatch.turnRef, status: "settled" },
    ]);

    const page1 = await store.readEvents({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      afterSequence: 0,
      limit: 2,
    });
    expect(page1.events.map((item) => item.sequence)).toEqual([1, 2]);
    const page2 = await store.readEvents({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      afterSequence: page1.nextSequence,
      limit: 10,
    });
    expect(page2.events.map((item) => item.sequence)).toEqual([3, 4, 5, 6]);

    expect(
      await store.readProjection({
        ownerRef,
        tenantRef,
        sandboxRef: `sandbox.sbx01.${suffix}`,
        translatorRef: "openagents.box_v1_translator.v1",
      }),
    ).toBeUndefined();

    const projection = await store.advanceProjection({
      ownerRef,
      tenantRef,
      sandboxRef: `sandbox.sbx01.${suffix}`,
      expectedProjectionVersion: 0,
      cursor: {
        translatorRef: "openagents.box_v1_translator.v1",
        nativeEventSequence: 2,
        boxCursor: "box.cursor.sbx01.events.2",
        omittedNativeKinds: ["native.cleanup.receipt"],
      },
      observedAt: observed(8),
    });
    expect(projection.projectionVersion).toBe(1);
    expect(
      await store.readProjection({
        ownerRef,
        tenantRef,
        sandboxRef: `sandbox.sbx01.${suffix}`,
        translatorRef: "openagents.box_v1_translator.v1",
      }),
    ).toEqual(projection);
    await expect(
      store.advanceProjection({
        ownerRef,
        tenantRef,
        sandboxRef: `sandbox.sbx01.${suffix}`,
        expectedProjectionVersion: 1,
        cursor: { ...projection.cursor, nativeEventSequence: 7 },
        observedAt: observed(9),
      }),
    ).rejects.toMatchObject({ code: "cursor_conflict" });
  });

  test("replays ordered provider events and interrupts only the exact generation-fenced turn", async () => {
    const suffix = "runtime";
    const sandboxRef = `sandbox.sbx01.${suffix}`;
    const turnRef = "turn.sbx04.runtime.1";
    await ready(suffix);
    const dispatch = {
      ...commandBase(suffix, "Dispatch", 2),
      _tag: "Dispatch" as const,
      turnRef,
      capabilityRef: `capability.sbx01.${suffix}.turn`,
      promptDigest: sha("f"),
      runtime: { ...runtime, provider: "claude" as const, modelRef: "model.claude.sonnet" },
    };
    await store.reserve({ command: dispatch });
    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef,
      commandRef: dispatch.commandRef,
      expectedResourceGeneration: 1,
      events: [event(suffix, "RuntimeStarted", 1, 3, { turnRef, turnEventSequence: 1 })],
      outcome: "succeeded",
      observedAt: observed(20),
    });

    const providerEvents = [
      {
        _tag: "RuntimeTextDelta",
        turnRef,
        resourceGeneration: 1,
        turnEventSequence: 2,
        content: "working",
        observedAt: observed(21),
      },
      {
        _tag: "RuntimeToolStarted",
        turnRef,
        resourceGeneration: 1,
        turnEventSequence: 3,
        toolCallRef: "tool.sbx04.runtime.1",
        toolName: "shell",
        observedAt: observed(22),
      },
      {
        _tag: "RuntimeToolCompleted",
        turnRef,
        resourceGeneration: 1,
        turnEventSequence: 4,
        toolCallRef: "tool.sbx04.runtime.1",
        toolName: "shell",
        outcome: "succeeded",
        evidenceRefs: ["evidence.sbx04.runtime.tool.1"],
        observedAt: observed(23),
      },
      {
        _tag: "RuntimeUsageRecorded",
        turnRef,
        resourceGeneration: 1,
        turnEventSequence: 5,
        usage: {
          inputTokens: 12,
          outputTokens: 7,
          cachedInputTokens: 4,
          providerUsageRef: "usage.sbx04.runtime.1",
          exact: true,
        },
        observedAt: observed(24),
      },
    ];
    const recorded = await store.recordRuntimeEvents({
      ownerRef,
      tenantRef,
      sandboxRef,
      turnRef,
      expectedResourceGeneration: 1,
      events: providerEvents,
    });
    expect(recorded.events.map((item) => item.sequence)).toEqual([4, 5, 6, 7]);
    expect(recorded.turn).toMatchObject({ status: "running", lastEventSequence: 5 });

    const replay = await store.recordRuntimeEvents({
      ownerRef,
      tenantRef,
      sandboxRef,
      turnRef,
      expectedResourceGeneration: 1,
      events: providerEvents,
    });
    expect(replay.events).toEqual([]);
    await expect(
      store.recordRuntimeEvents({
        ownerRef,
        tenantRef,
        sandboxRef,
        turnRef,
        expectedResourceGeneration: 1,
        events: [{ ...providerEvents[0], content: "changed bytes" }],
      }),
    ).rejects.toMatchObject({ code: "event_conflict" });

    const reconnect = await store.readTurnEvents({
      ownerRef,
      tenantRef,
      sandboxRef,
      turnRef,
      afterTurnSequence: 2,
      limit: 10,
    });
    expect(
      reconnect.events.map((item) =>
        "turnEventSequence" in item ? item.turnEventSequence : undefined,
      ),
    ).toEqual([3, 4, 5]);

    const interrupt = {
      ...commandBase(suffix, "Interrupt", 5),
      _tag: "Interrupt" as const,
      turnRef,
      reasonRef: "reason.sbx04.operator.stop",
    };
    await store.reserve({ command: interrupt });
    await store.settle({
      ownerRef,
      tenantRef,
      sandboxRef,
      commandRef: interrupt.commandRef,
      expectedResourceGeneration: 1,
      events: [
        event(suffix, "RuntimeInterruptRequested", 1, 8, {
          turnRef,
          turnEventSequence: 6,
          reasonRef: interrupt.reasonRef,
        }),
      ],
      outcome: "succeeded",
      observedAt: observed(25),
    });
    expect(
      (await store.inspectTurn({ ownerRef, tenantRef, sandboxRef, turnRef })).turn,
    ).toMatchObject({ status: "interrupting", lastEventSequence: 6 });

    const terminal = await store.recordRuntimeEvents({
      ownerRef,
      tenantRef,
      sandboxRef,
      turnRef,
      expectedResourceGeneration: 1,
      events: [
        {
          _tag: "RuntimeInterrupted",
          turnRef,
          resourceGeneration: 1,
          turnEventSequence: 7,
          reasonRef: interrupt.reasonRef,
          observedAt: observed(26),
        },
      ],
      evidenceRefs: ["evidence.sbx04.runtime.interrupt.1"],
    });
    expect(terminal.turn).toMatchObject({ status: "interrupted", terminalReason: "explicit_stop" });
    expect(terminal.receipt).toMatchObject({
      outcome: "interrupted",
      terminalEventSequence: 7,
      runtime: { provider: "claude", modelRef: "model.claude.sonnet" },
    });
    expect((await store.inspect({ ownerRef, tenantRef, sandboxRef })).facts).toMatchObject({
      lifecycle: "idle",
      runtimeState: "settled",
    });
    await expect(
      store.recordRuntimeEvents({
        ownerRef,
        tenantRef,
        sandboxRef,
        turnRef,
        expectedResourceGeneration: 2,
        events: [],
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
  });

  test("persists bounded lease and budget updates and refuses cross-owner reads", async () => {
    const suffix = "update";
    await ready(suffix);
    const update = {
      ...commandBase(suffix, "Update", 2),
      _tag: "Update" as const,
      lease: lease(`${suffix}.renewed`, 120),
      budget: { ...budget, maxCostMicros: 500_000 },
    };
    const settled = await store.reserve({ command: update });
    expect(settled).toMatchObject({ disposition: "settled", status: "settled" });
    expect(settled.resource).toMatchObject({
      version: 3,
      budget: { maxCostMicros: 500_000 },
      lease: { expiresAt: observed(120) },
    });
    const attenuated = await store.reserve({
      command: {
        ...commandBase(suffix, "Update", 3),
        _tag: "Update" as const,
        lease: lease(`${suffix}.attenuated`, 30),
        budget: {
          ...budget,
          maxCostMicros: 500_000,
          maxLifetimeSeconds: 1_800,
        },
        capabilities: settled.resource.capabilities.map((capability) => ({
          ...capability,
          expiresAt: observed(30),
        })),
      },
    });
    expect(attenuated.resource).toMatchObject({
      version: 4,
      lease: { expiresAt: observed(30), ttlSeconds: 1_800 },
      capabilities: [{ expiresAt: observed(30) }],
    });
    expect(await store.expired({ ownerRef, tenantRef, at: observed(29) })).not.toContainEqual(
      expect.objectContaining({ sandboxRef: `sandbox.sbx01.${suffix}` }),
    );
    expect(await store.expired({ ownerRef, tenantRef, at: observed(30) })).toContainEqual(
      expect.objectContaining({ sandboxRef: `sandbox.sbx01.${suffix}` }),
    );
    await expect(
      store.inspect({
        ownerRef: "owner.sbx01.other",
        tenantRef,
        sandboxRef: `sandbox.sbx01.${suffix}`,
      }),
    ).rejects.toBeInstanceOf(ManagedSandboxStoreError);
  });
});
