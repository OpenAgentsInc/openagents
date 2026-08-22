import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import { SQL } from "@openagentsinc/postgres-runtime";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  CloudComputerCommandStoreError,
  PostgresCloudComputerCommandStore,
} from "./cloud-computer-command-store.js";
import {
  persistCloudComputerCommandOutput,
  type CloudComputerCommandArtifactObject,
} from "./cloud-computer-command-artifact.js";
import {
  cloudComputerReverseDialCredentialAuthority,
  createCloudComputerCommand,
  createCloudComputerCommandEvent,
  CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA,
  CloudComputerReverseDialSession,
} from "./cloud-computer-command.js";
import { cloudComputerCommandRecoveryEvidenceAuthority } from "./cloud-computer-command-recovery.js";
import { runMigrations } from "./migrate.js";
import type { SyncSql } from "./sql.js";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js";

const at = (seconds: number): string =>
  new Date(Date.UTC(2026, 7, 22, 21, 0, seconds)).toISOString();
const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const requestDigest = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;
const eventDigest = (
  input: Omit<
    Parameters<typeof createCloudComputerCommandEvent>[0],
    "sessionRef" | "runtimeRef" | "runtimeGeneration"
  >,
): string =>
  createCloudComputerCommandEvent({
    ...input,
    sessionRef: "session.command.main",
    runtimeRef: "runtime.command.one",
    runtimeGeneration: 1,
  }).eventDigest;

describe.skipIf(!hasLocalPostgres())("cloud computer command Postgres authority", () => {
  let pg: LocalPostgres;
  let sql: SQL;
  let store: PostgresCloudComputerCommandStore;
  let attachmentEpoch: number;
  const credentialAuthority = cloudComputerReverseDialCredentialAuthority({
    authorize: async () => true,
  });
  const recoveryAuthority = cloudComputerCommandRecoveryEvidenceAuthority({
    verify: async () => true,
  });

  const scope = {
    sessionRef: "session.command.main",
    computerRef: "computer.command.main",
    workspaceRef: "workspace.command.main",
    ownerRef: "owner.command.main",
    tenantRef: "tenant.command.main",
    runtimeGeneration: 1,
    runtimeRef: "runtime.command.one",
    providerLeaseRef: "lease.command.one",
  } as const;

  const fence = (commandRef: string, observedAt: string) => ({
    commandRef,
    sessionRef: scope.sessionRef,
    attachmentEpoch,
    runtimeGeneration: 1,
    runtimeRef: scope.runtimeRef,
    providerLeaseRef: scope.providerLeaseRef,
    observedAt,
  });

  const credential = (nonce: string, authorityDigest = digest("a") as `sha256:${string}`) =>
    credentialAuthority.issue({
      ...scope,
      nonce,
      authorityDigest,
      issuedAt: at(0),
      expiresAt: at(20),
    });

  const admit = async (suffix: string, deadlineAt = at(50), argument = suffix) => {
    const timeoutMs = Date.parse(deadlineAt) - Date.parse(at(1));
    const request = createCloudComputerCommand({
      commandRef: `command.command.${suffix}`,
      idempotencyRef: `idempotency.command.${suffix}`,
      ownerRef: scope.ownerRef,
      tenantRef: scope.tenantRef,
      computerRef: scope.computerRef,
      workspaceRef: scope.workspaceRef,
      runtimeRef: scope.runtimeRef,
      runtimeGeneration: 1,
      providerLeaseRef: scope.providerLeaseRef,
      kind: "exec",
      argv: ["/usr/bin/printf", argument],
      workingDirectory: "/workspace",
      environmentDigest: digest("e") as `sha256:${string}`,
      issuedAt: at(1),
      deadlineAt,
      timeoutMs,
      capabilityRefs: ["capability.command.execute"],
      capabilitySnapshotDigest: digest("c") as `sha256:${string}`,
      authorityDigest: digest("a") as `sha256:${string}`,
      budgetSnapshotDigest: digest("b") as `sha256:${string}`,
      budget: {
        outputBytes: 1_024,
        wallTimeMs: Math.min(30_000, timeoutMs),
        cpuTimeMs: Math.min(30_000, timeoutMs),
      },
    });
    return store.admit({
      commandRef: request.commandRef,
      idempotencyRef: request.idempotencyRef,
      requestDigest: request.requestDigest,
      computerRef: scope.computerRef,
      workspaceRef: scope.workspaceRef,
      sessionRef: scope.sessionRef,
      ownerRef: scope.ownerRef,
      tenantRef: scope.tenantRef,
      runtimeGeneration: 1,
      runtimeRef: scope.runtimeRef,
      providerLeaseRef: scope.providerLeaseRef,
      workingDirectory: request.workingDirectory,
      capabilityRefs: request.capabilityRefs,
      capabilityDigest: request.capabilitySnapshotDigest,
      budgetSnapshotDigest: request.budgetSnapshotDigest,
      budgetLimits: request.budget,
      deadlineAt,
      request,
      createdAt: at(1),
    });
  };

  const dispatch = async (suffix: string) => {
    const commandRef = `command.command.${suffix}`;
    const dispatchRef = `dispatch.command.${suffix}`;
    await store.prepareDispatchAttempt({ ...fence(commandRef, at(2)), dispatchRef });
    return store.exposeDispatchAttempt({ ...fence(commandRef, at(3)), dispatchRef });
  };

  const running = async (suffix: string) => {
    const command = await store.get(`command.command.${suffix}`);
    const coreCommand = (await store.loadCommandForDispatch(command.commandRef))?.command;
    if (coreCommand === undefined) throw new Error("expected dispatch command");
    const runtimeSession = new CloudComputerReverseDialSession({
      ...scope,
      authorityDigest: digest("a") as `sha256:${string}`,
    });
    runtimeSession.admit(coreCommand);
    const runtimeAttachment = await runtimeSession.attach({
      connectionRef: `connection.runtime.${suffix}`,
      credential: await credential(`nonce.runtime.${suffix}`),
      now: at(1),
      cursors: [],
    });
    await dispatch(suffix);
    const reservation = runtimeSession.reserve({
      attachmentEpoch: runtimeAttachment.attachmentEpoch,
      commandRef: command.commandRef,
      requestDigest: command.requestDigest as `sha256:${string}`,
      reservationRef: `reservation.command.${suffix}`,
      providerExecutionRef: `execution.command.${suffix}`,
      reservedAt: at(4),
    }).reservation;
    await store.recordReservation({
      ...fence(`command.command.${suffix}`, at(4)),
      dispatchRef: `dispatch.command.${suffix}`,
      reservationRef: `reservation.command.${suffix}`,
      providerExecutionRef: `execution.command.${suffix}`,
      providerCommandRef: `provider-command.command.${suffix}`,
      reservation,
    });
    const runtimeAcknowledgement = runtimeSession.acknowledge({
      attachmentEpoch: runtimeAttachment.attachmentEpoch,
      commandRef: command.commandRef,
      requestDigest: command.requestDigest as `sha256:${string}`,
      reservationRef: reservation.reservationRef,
      providerExecutionRef: reservation.providerExecutionRef,
      eventRef: `event.command.${suffix}.accepted`,
      sequence: 1,
      observedAt: at(4),
    });
    const acknowledgement = await store.recordDispatchedAcknowledgement({
      ...fence(`command.command.${suffix}`, at(4)),
      dispatchRef: `dispatch.command.${suffix}`,
      reservationRef: `reservation.command.${suffix}`,
      providerExecutionRef: `execution.command.${suffix}`,
      providerCommandRef: `provider-command.command.${suffix}`,
      acknowledgementEventRef: `event.command.${suffix}.accepted`,
      acknowledgementEventDigest: eventDigest({
        eventRef: `event.command.${suffix}.accepted`,
        commandRef: `command.command.${suffix}`,
        requestDigest: command.requestDigest as `sha256:${string}`,
        providerExecutionRef: `execution.command.${suffix}`,
        sequence: 1,
        fence: 1,
        kind: "accepted",
        payload: {
          reservationRef: `reservation.command.${suffix}`,
          providerExecutionRef: `execution.command.${suffix}`,
        },
        observedAt: at(4),
      }),
      expectedFence: 1,
      expectedCommandSequence: 1,
      reservation,
      acknowledgement: runtimeAcknowledgement,
    });
    expect(acknowledgement.status).toBe("dispatched");
    return store.recordRunning({
      ...fence(`command.command.${suffix}`, at(5)),
      dispatchRef: `dispatch.command.${suffix}`,
      providerCommandRef: `provider-command.command.${suffix}`,
      providerExecutionRef: `execution.command.${suffix}`,
      requestDigest: command.requestDigest,
      expectedFence: 1,
    });
  };

  beforeAll(async () => {
    pg = await startLocalPostgres();
    const databaseName = `khala_sync_cloud_command_${process.pid}_${Date.now()}`;
    const admin = SQL({ url: pg.url, max: 1 });
    await admin.unsafe(`CREATE DATABASE ${databaseName}`);
    await admin.end();
    await runMigrations({ databaseUrl: pg.urlFor(databaseName) });
    sql = SQL({ url: pg.urlFor(databaseName), max: 12 });
    store = new PostgresCloudComputerCommandStore(sql as unknown as SyncSql);
    await sql`
      INSERT INTO khala_sync_cloud_computers
        (computer_ref, owner_ref, tenant_ref, conversation_ref, work_unit_ref,
         kind, runtime_class, generation, version, runtime_profile_ref,
         authority_snapshot_digest, budget_snapshot_digest, capability_refs,
         state, active_lease_ref, created_at, updated_at)
      VALUES (${scope.computerRef}, ${scope.ownerRef}, ${scope.tenantRef},
              'conversation.command.main', 'work.command.main', 'interactive_retained',
              'standard', 1, 1, 'profile.command.standard', ${digest("a")}, ${digest("b")},
              '["capability.command.execute"]'::jsonb, 'active', ${scope.providerLeaseRef},
              ${at(0)}, ${at(0)})
    `;
    await sql`
      INSERT INTO khala_sync_cloud_computer_workspaces
        (workspace_ref, computer_ref, runtime_generation, owner_ref, tenant_ref,
         conversation_ref, base_image_digest, base_image_signature_ref,
         workspace_key_ref, workspace_key_version, created_at, updated_at)
      VALUES (${scope.workspaceRef}, ${scope.computerRef}, 1, ${scope.ownerRef}, ${scope.tenantRef},
              'conversation.command.main', ${digest("d")}, 'signature.command.base',
              'key.command.workspace', 1, ${at(0)}, ${at(0)})
    `;
    await store.initializeSession({ ...scope, createdAt: at(0) });
    const attachment = await store.attach({
      connectionRef: "connection.command.one",
      credential: await credential("nonce.command.one"),
      observedAt: at(1),
    });
    attachmentEpoch = attachment.attachmentEpoch;
  });

  afterAll(async () => {
    if (sql !== undefined) await sql.end();
    if (pg !== undefined) await pg.stop();
  });

  test("binds exact admission and persists write exposure before provider I/O", async () => {
    const admitted = await admit("uncertain");
    expect(admitted.status).toBe("admitted");
    expect((await admit("uncertain")).replayed).toBe(true);
    await expect(admit("uncertain", at(50), "different")).rejects.toMatchObject({
      code: "conflict",
    });

    await store.prepareDispatchAttempt({
      ...fence(admitted.commandRef, at(2)),
      dispatchRef: "dispatch.command.uncertain",
    });
    const prepared: ReadonlyArray<{ status: string }> = await sql`
      SELECT status FROM khala_sync_cloud_computer_command_dispatch_attempts
      WHERE attempt_ref = 'dispatch.command.uncertain'
    `;
    expect(prepared[0]?.status).toBe("prepared");
    expect(
      (await store.recoverSafeToDispatch()).find(
        (command) => command.commandRef === admitted.commandRef,
      )?.dispatchRef,
    ).toBe("dispatch.command.uncertain");
    expect(await store.getDispatchAttempt(admitted.commandRef)).toMatchObject({
      dispatchRef: "dispatch.command.uncertain",
      status: "prepared",
      reservationRef: null,
    });
    const recovered = await store.loadCommandForDispatch(admitted.commandRef);
    expect(recovered?.dispatchRef).toBe("dispatch.command.uncertain");
    expect(recovered?.requestDigest).toBe(admitted.requestDigest);
    expect(recovered?.command).toMatchObject({
      commandRef: admitted.commandRef,
      argv: ["/usr/bin/printf", "uncertain"],
      requestDigest: admitted.requestDigest,
    });
    await store.prepareDispatchAttempt({
      ...fence(admitted.commandRef, at(2)),
      dispatchRef: "dispatch.command.uncertain",
    });
    await expect(
      store.prepareDispatchAttempt({
        ...fence(admitted.commandRef, at(2)),
        dispatchRef: "dispatch.command.second",
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    const claim = await store.exposeDispatchAttempt({
      ...fence(admitted.commandRef, at(3)),
      dispatchRef: "dispatch.command.uncertain",
    });
    expect(claim.authentic()).toBe(true);
    expect((await store.get(admitted.commandRef)).status).toBe("may_have_started");
    await expect(
      store.recordRunning({
        ...fence(admitted.commandRef, at(3)),
        runtimeGeneration: 2,
        dispatchRef: "dispatch.command.uncertain",
        providerCommandRef: "provider-command.command.wrong-generation",
        providerExecutionRef: "execution.command.uncertain",
        requestDigest: admitted.requestDigest,
        expectedFence: 1,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    await expect(
      store.recordRunning({
        ...fence(admitted.commandRef, at(3)),
        providerLeaseRef: "lease.command.wrong",
        dispatchRef: "dispatch.command.uncertain",
        providerCommandRef: "provider-command.command.wrong-lease",
        providerExecutionRef: "execution.command.uncertain",
        requestDigest: admitted.requestDigest,
        expectedFence: 1,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    await expect(
      store.exposeDispatchAttempt({
        ...fence(admitted.commandRef, at(3)),
        dispatchRef: "dispatch.command.uncertain",
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    expect((await store.recoverUnsettled()).map((command) => command.commandRef)).toContain(
      admitted.commandRef,
    );
    await store.markMayHaveStarted({
      ...fence(admitted.commandRef, at(4)),
      dispatchRef: "dispatch.command.uncertain",
    });
    await expect(
      store.prepareDispatchAttempt({
        ...fence(admitted.commandRef, at(5)),
        dispatchRef: "dispatch.command.replay",
      }),
    ).rejects.toMatchObject({ code: "conflict" });
  });

  test("fences attachments and keeps dense command plus session event sequences", async () => {
    const command = await admit("events");
    await running("events");
    await expect(
      store.recordArtifact({
        ...fence(command.commandRef, at(5)),
        kind: "stdout",
        artifact: {
          storage: "artifact",
          artifactRef: "artifact.command.forged",
          object: {
            objectRef: "object.command.forged",
            generation: "1",
            contentDigest: digest("e") as `sha256:${string}`,
            byteCount: 128,
          },
          contentDigest: digest("e") as `sha256:${string}`,
          byteCount: 128,
          retainUntil: at(50),
          reused: false,
        },
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
      }),
    ).rejects.toThrow("artifact was not issued by the output service");
    let artifactObject: CloudComputerCommandArtifactObject | null = null;
    const artifact = await persistCloudComputerCommandOutput({
      storage: {
        inspect: async () => artifactObject,
        createOnly: async (input) => {
          artifactObject = {
            objectRef: input.objectRef,
            generation: "17",
            contentDigest: input.contentDigest,
            byteCount: input.bytes.byteLength,
          };
          return artifactObject;
        },
        download: async () => new Uint8Array(),
      },
      ownerRef: scope.ownerRef,
      tenantRef: scope.tenantRef,
      commandRef: command.commandRef,
      runtimeGeneration: 1,
      kind: "stdout",
      bytes: new Uint8Array(128).fill(1),
      inlineByteLimit: 32,
      commandByteLimit: 1_024,
      priorCommandByteCount: 0,
      retainUntil: at(50),
    });
    if (artifact.storage !== "artifact") throw new Error("expected artifact output");
    await store.recordArtifact({
      ...fence(command.commandRef, at(5)),
      kind: "stdout",
      artifact,
      requestDigest: command.requestDigest,
      providerExecutionRef: "execution.command.events",
      providerCommandRef: "provider-command.command.events",
      expectedFence: 1,
    });
    const oversizedArtifact = await persistCloudComputerCommandOutput({
      storage: {
        inspect: async () => null,
        createOnly: async (input) => ({
          objectRef: input.objectRef,
          generation: "18",
          contentDigest: input.contentDigest,
          byteCount: input.bytes.byteLength,
        }),
        download: async () => new Uint8Array(),
      },
      ownerRef: scope.ownerRef,
      tenantRef: scope.tenantRef,
      commandRef: command.commandRef,
      runtimeGeneration: 1,
      kind: "stderr",
      bytes: new Uint8Array(950).fill(2),
      inlineByteLimit: 32,
      commandByteLimit: 1_024,
      priorCommandByteCount: 0,
      retainUntil: at(50),
    });
    if (oversizedArtifact.storage !== "artifact") throw new Error("expected artifact output");
    await expect(
      store.recordArtifact({
        ...fence(command.commandRef, at(5)),
        kind: "stderr",
        artifact: oversizedArtifact,
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    await expect(
      store.appendEvent({
        ...fence(command.commandRef, at(5)),
        eventRef: "event.command.events.oversized",
        eventDigest: digest("f"),
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
        expectedCommandSequence: 2,
        kind: "progress",
        payload: { chunk: "x".repeat(70_000) },
        artifactRefs: [],
        maxRetainedEvents: 2,
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    const sequences = [];
    for (let index = 1; index <= 3; index += 1) {
      // eslint-disable-next-line no-await-in-loop -- verifies committed sequence order.
      const sequence = await store.appendEvent({
        ...fence(command.commandRef, at(5 + index)),
        eventRef: `event.command.events.${index}`,
        eventDigest: eventDigest({
          eventRef: `event.command.events.${index}`,
          commandRef: command.commandRef,
          requestDigest: command.requestDigest as `sha256:${string}`,
          providerExecutionRef: "execution.command.events",
          sequence: index + 1,
          fence: 1,
          kind: "stdout",
          payload: { chunk: index },
          observedAt: at(5 + index),
        }),
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
        expectedCommandSequence: index + 1,
        kind: "stdout",
        payload: { chunk: index },
        artifactRefs: index === 1 ? [artifact.artifactRef] : [],
        maxRetainedEvents: 2,
      });
      sequences.push(sequence);
    }
    expect(sequences.map((sequence) => sequence.commandSequence)).toEqual([2, 3, 4]);
    expect(sequences.map((sequence) => sequence.sessionSequence)).toEqual([2, 3, 4]);
    expect(
      await store.appendEvent({
        ...fence(command.commandRef, at(8)),
        eventRef: "event.command.events.3",
        eventDigest: eventDigest({
          eventRef: "event.command.events.3",
          commandRef: command.commandRef,
          requestDigest: command.requestDigest as `sha256:${string}`,
          providerExecutionRef: "execution.command.events",
          sequence: 4,
          fence: 1,
          kind: "stdout",
          payload: { chunk: 3 },
          observedAt: at(8),
        }),
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
        expectedCommandSequence: 4,
        kind: "stdout",
        payload: { chunk: 3 },
        artifactRefs: [],
        maxRetainedEvents: 2,
      }),
    ).toEqual(sequences[2]);
    await expect(
      store.appendEvent({
        ...fence(command.commandRef, at(8)),
        eventRef: "event.command.events.conflict",
        eventDigest: digest("3"),
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
        expectedCommandSequence: 4,
        kind: "stdout",
        payload: { chunk: 3 },
        artifactRefs: [],
        maxRetainedEvents: 2,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    const session: ReadonlyArray<{
      retained_through_session_sequence: string | number;
      retention_epoch: string | number;
    }> = await sql`
      SELECT retained_through_session_sequence, retention_epoch
      FROM khala_sync_cloud_computer_command_sessions WHERE session_ref = ${scope.sessionRef}
    `;
    expect(Number(session[0]?.retained_through_session_sequence)).toBe(2);
    expect(Number(session[0]?.retention_epoch)).toBe(2);

    const nextCredential = await credential("nonce.command.two");
    await expect(
      store.attach({
        connectionRef: "connection.command.wrong-authority",
        credential: await credential(
          "nonce.command.wrong-authority",
          digest("f") as `sha256:${string}`,
        ),
        observedAt: at(10),
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    const nextAttachment = await store.attach({
      connectionRef: "connection.command.two",
      credential: nextCredential,
      observedAt: at(10),
    });
    await expect(
      store.attach({
        connectionRef: "connection.command.replay",
        credential: nextCredential,
        observedAt: at(10),
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    await expect(
      store.appendEvent({
        ...fence(command.commandRef, at(11)),
        eventRef: "event.command.events.stale",
        eventDigest: digest("9"),
        requestDigest: command.requestDigest,
        providerExecutionRef: "execution.command.events",
        providerCommandRef: "provider-command.command.events",
        expectedFence: 1,
        expectedCommandSequence: 5,
        kind: "progress",
        payload: {},
        artifactRefs: [],
        maxRetainedEvents: 2,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
    attachmentEpoch = nextAttachment.attachmentEpoch;
  });

  test("persists completed, failed, cancelled, and timed-out terminal outcomes exactly", async () => {
    for (const suffix of ["complete", "failed", "cancel", "timeout"]) {
      // eslint-disable-next-line no-await-in-loop -- each command exercises one terminal state.
      await admit(suffix, suffix === "timeout" ? at(6) : at(50));
      // eslint-disable-next-line no-await-in-loop -- dispatch ordering is the contract under test.
      await running(suffix);
    }
    const terminal = async (
      suffix: string,
      status: "completed" | "failed" | "cancelled" | "timed_out",
      observedAt: string,
    ) => {
      const command = await store.get(`command.command.${suffix}`);
      const expectedFence = status === "cancelled" || status === "timed_out" ? 2 : 1;
      const exitCode = status === "completed" ? 0 : status === "failed" ? 1 : null;
      const outputDigest = status === "completed" ? digest("e") : null;
      const reason =
        status === "cancelled"
          ? "user_cancelled"
          : status === "timed_out"
            ? "deadline_elapsed"
            : status;
      const terminalRef = `terminal.command.${suffix}`;
      const terminalEventDigest = eventDigest({
        eventRef: terminalRef,
        commandRef: command.commandRef,
        requestDigest: command.requestDigest as `sha256:${string}`,
        providerExecutionRef: `execution.command.${suffix}`,
        sequence: 2,
        fence: expectedFence,
        kind: status,
        payload: { exitCode, outputDigest, reason },
        observedAt,
      });
      const terminalDigest = requestDigest({
        schema: CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA,
        terminalRef,
        commandRef: command.commandRef,
        requestDigest: command.requestDigest,
        providerExecutionRef: `execution.command.${suffix}`,
        sessionRef: scope.sessionRef,
        runtimeRef: scope.runtimeRef,
        runtimeGeneration: 1,
        fence: expectedFence,
        sequence: 2,
        outcome: status,
        exitCode,
        outputDigest,
        reason,
        observedAt,
        eventDigest: terminalEventDigest,
      });
      return store.recordTerminal({
        ...fence(`command.command.${suffix}`, observedAt),
        providerCommandRef: `provider-command.command.${suffix}`,
        terminalRef,
        terminalDigest,
        eventDigest: terminalEventDigest,
        status,
        reason,
        exitCode,
        outputDigest,
        artifactRefs: [],
        maxRetainedEvents: 20,
        requestDigest: command.requestDigest,
        providerExecutionRef: `execution.command.${suffix}`,
        expectedFence,
        expectedCommandSequence: 2,
      });
    };
    const completeCommand = await store.get("command.command.complete");
    await expect(
      store.recordTerminal({
        ...fence("command.command.complete", at(7)),
        providerCommandRef: "provider-command.command.complete",
        terminalRef: "terminal.command.complete.nonzero",
        terminalDigest: digest("4"),
        eventDigest: digest("4"),
        status: "completed",
        reason: "completed",
        exitCode: 9,
        outputDigest: digest("e"),
        artifactRefs: [],
        maxRetainedEvents: 20,
        requestDigest: completeCommand.requestDigest,
        providerExecutionRef: "execution.command.complete",
        expectedFence: 1,
        expectedCommandSequence: 2,
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    await expect(
      store.recordTerminal({
        ...fence("command.command.failed", at(7)),
        providerCommandRef: "provider-command.command.failed",
        terminalRef: "terminal.command.failed.unsafe-reason",
        terminalDigest: digest("5"),
        eventDigest: digest("5"),
        status: "failed",
        reason: "Process failed",
        exitCode: 1,
        outputDigest: null,
        artifactRefs: [],
        maxRetainedEvents: 20,
        requestDigest: (await store.get("command.command.failed")).requestDigest,
        providerExecutionRef: "execution.command.failed",
        expectedFence: 1,
        expectedCommandSequence: 2,
      }),
    ).rejects.toMatchObject({ code: "invalid" });
    const completed = await terminal("complete", "completed", at(7));
    expect(completed.status).toBe("completed");
    expect(completed.terminalSessionSequence).not.toBeNull();
    expect(completed.terminalCommandSequence).toBe(2);
    const terminalPayloads: ReadonlyArray<{ payload_json: unknown }> = await sql`
      SELECT payload_json FROM khala_sync_cloud_computer_command_events
      WHERE event_ref = 'terminal.command.complete'
    `;
    expect(terminalPayloads[0]?.payload_json).toEqual({
      exitCode: 0,
      outputDigest: digest("e"),
      reason: "completed",
    });
    expect((await terminal("failed", "failed", at(7))).status).toBe("failed");
    const cancellation = {
      ...fence("command.command.cancel", at(7)),
      settlementRef: "settlement.command.cancel",
      settlementRequestDigest: digest("7"),
      expectedFence: 1,
      reason: "user_cancelled",
    };
    expect((await store.requestCancellation(cancellation)).fence).toBe(2);
    expect((await store.requestCancellation(cancellation)).replayed).toBe(true);
    await expect(
      store.requestCancellation({
        ...fence("command.command.timeout", at(7)),
        settlementRef: cancellation.settlementRef,
        settlementRequestDigest: cancellation.settlementRequestDigest,
        expectedFence: 1,
        reason: cancellation.reason,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    await expect(
      store.recordTerminal({
        ...fence("command.command.cancel", at(8)),
        providerCommandRef: "provider-command.command.cancel",
        terminalRef: "terminal.command.cancel.stale-complete",
        terminalDigest: digest("d"),
        eventDigest: digest("d"),
        status: "completed",
        reason: "stale_completion",
        exitCode: 0,
        outputDigest: digest("d"),
        artifactRefs: [],
        maxRetainedEvents: 20,
        requestDigest: (await store.get("command.command.cancel")).requestDigest,
        providerExecutionRef: "execution.command.cancel",
        expectedFence: 1,
        expectedCommandSequence: 2,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    expect((await terminal("cancel", "cancelled", at(8))).status).toBe("cancelled");
    const timeout = {
      ...fence("command.command.timeout", at(7)),
      settlementRef: "settlement.command.timeout",
      settlementRequestDigest: digest("8"),
      expectedFence: 1,
      reason: "deadline_elapsed",
    };
    expect((await store.requestTimeout(timeout)).fence).toBe(2);
    expect((await terminal("timeout", "timed_out", at(8))).status).toBe("timed_out");
    const replay = await terminal("complete", "completed", at(7));
    expect(replay.replayed).toBe(true);
    expect(replay.terminalSessionSequence).toBe(completed.terminalSessionSequence);
    await expect(
      store.recordTerminal({
        ...fence("command.command.complete", at(7)),
        providerCommandRef: "provider-command.command.complete",
        terminalRef: "terminal.command.complete",
        terminalDigest: digest("f"),
        eventDigest: digest("f"),
        status: "completed",
        reason: "completed",
        exitCode: 0,
        outputDigest: digest("f"),
        artifactRefs: [],
        maxRetainedEvents: 20,
        requestDigest: completed.requestDigest,
        providerExecutionRef: "execution.command.complete",
        expectedFence: 1,
        expectedCommandSequence: 2,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
    await expect(
      store.appendEvent({
        ...fence("command.command.complete", at(9)),
        eventRef: "event.command.complete.after-terminal",
        eventDigest: digest("f"),
        requestDigest: completed.requestDigest,
        providerExecutionRef: "execution.command.complete",
        providerCommandRef: "provider-command.command.complete",
        expectedFence: 1,
        expectedCommandSequence: 3,
        kind: "progress",
        payload: {},
        artifactRefs: [],
        maxRetainedEvents: 20,
      }),
    ).rejects.toMatchObject({ code: "conflict" });
  });

  test("uses durable host-loss evidence to settle exposed and unexposed commands", async () => {
    const checkpointEvidence = await recoveryAuthority.issue({
      kind: "checkpoint_failed",
      evidenceRef: "evidence.command.checkpoint",
      evidenceDigest: digest("c"),
      computerRef: scope.computerRef,
      workspaceRef: scope.workspaceRef,
      runtimeGeneration: 1,
      runtimeRef: scope.runtimeRef,
      providerLeaseRef: scope.providerLeaseRef,
      observedAt: at(11),
      durable: true,
    });
    expect(await store.recordRecoveryEvidence(checkpointEvidence)).toEqual({
      lost: 0,
      replayed: false,
    });
    expect(await store.recordRecoveryEvidence(checkpointEvidence)).toEqual({
      lost: 0,
      replayed: true,
    });
    const cleanupEvidence = await recoveryAuthority.issue({
      kind: "cleanup_failed",
      evidenceRef: "evidence.command.cleanup",
      evidenceDigest: digest("b"),
      computerRef: scope.computerRef,
      workspaceRef: scope.workspaceRef,
      runtimeGeneration: 1,
      runtimeRef: scope.runtimeRef,
      providerLeaseRef: scope.providerLeaseRef,
      observedAt: at(11),
      durable: true,
    });
    expect(await store.recordRecoveryEvidence(cleanupEvidence)).toEqual({
      lost: 0,
      replayed: false,
    });
    const runtimeEvidence = await recoveryAuthority.issue({
      kind: "runtime_lost",
      evidenceRef: "evidence.command.runtime-loss",
      evidenceDigest: digest("d"),
      computerRef: scope.computerRef,
      workspaceRef: scope.workspaceRef,
      runtimeGeneration: 1,
      runtimeRef: scope.runtimeRef,
      providerLeaseRef: scope.providerLeaseRef,
      observedAt: at(11),
      durable: true,
    });
    const runtimeLoss = await store.recordRecoveryEvidence(runtimeEvidence);
    expect(runtimeLoss.lost).toBeGreaterThan(0);
    expect(await store.recordRecoveryEvidence(runtimeEvidence)).toEqual({
      lost: runtimeLoss.lost,
      replayed: true,
    });
    await admit("lost");
    await dispatch("lost");
    await admit("safe");
    await store.prepareDispatchAttempt({
      ...fence("command.command.safe", at(12)),
      dispatchRef: "dispatch.command.safe",
    });
    await sql.begin(async (tx) => {
      await tx`
        INSERT INTO khala_sync_cloud_computer_host_loss_evidence
          (evidence_ref, computer_ref, workspace_ref, owner_ref, tenant_ref,
           runtime_generation, provider_lease_ref, evidence_digest, observed_at)
        VALUES ('evidence.command.host-loss', ${scope.computerRef}, ${scope.workspaceRef},
                ${scope.ownerRef}, ${scope.tenantRef}, 1, ${scope.providerLeaseRef},
                ${digest("9")}, ${at(13)})
      `;
      await tx`
        UPDATE khala_sync_cloud_computers
        SET state = 'failed', active_lease_ref = NULL, updated_at = ${at(13)}
        WHERE computer_ref = ${scope.computerRef}
      `;
    });
    expect(
      await store.settleHostLoss({
        evidenceRef: "evidence.command.host-loss",
        evidenceDigest: digest("9"),
        computerRef: scope.computerRef,
        workspaceRef: scope.workspaceRef,
        runtimeGeneration: 1,
        runtimeRef: scope.runtimeRef,
        providerLeaseRef: scope.providerLeaseRef,
        observedAt: at(14),
      }),
    ).toEqual({ lost: 1, notDispatched: 1 });
    expect((await store.get("command.command.lost")).status).toBe("lost");
    expect((await store.get("command.command.safe")).status).toBe("not_dispatched");
    expect((await store.get("command.command.complete")).status).toBe("completed");
    await expect(
      store.appendEvent({
        ...fence("command.command.lost", at(15)),
        eventRef: "event.command.after-loss",
        eventDigest: digest("a"),
        requestDigest: (await store.get("command.command.lost")).requestDigest,
        providerExecutionRef: "execution.command.lost",
        providerCommandRef: "provider-command.command.lost",
        expectedFence: 1,
        expectedCommandSequence: 1,
        kind: "progress",
        payload: {},
        artifactRefs: [],
        maxRetainedEvents: 2,
      }),
    ).rejects.toMatchObject({ code: "stale_generation" });
  });

  test("uses typed errors", () => {
    expect(new CloudComputerCommandStoreError("conflict", "fixture")).toBeInstanceOf(Error);
  });
});
