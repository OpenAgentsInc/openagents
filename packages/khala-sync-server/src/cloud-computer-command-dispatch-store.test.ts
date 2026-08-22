import { describe, expect, test } from "vite-plus/test";

import {
  CloudComputerReverseDialSession,
  cloudComputerReverseDialCredentialAuthority,
  createCloudComputerCommand,
  type CloudComputerRuntimeAcknowledgement,
  type CloudComputerRuntimeReservation,
} from "./cloud-computer-command.js";
import { createCloudComputerCommandDispatcher } from "./cloud-computer-command-dispatch.js";
import { createPostgresCloudComputerDurableDispatchPort } from "./cloud-computer-command-dispatch-store.js";
import type {
  CloudComputerCommand as StoredCommand,
  CloudComputerCommandDispatchClaim,
  PostgresCloudComputerCommandStore,
} from "./cloud-computer-command-store.js";

const sha = (value: string): `sha256:${string}` => `sha256:${value.repeat(64)}`;
const command = createCloudComputerCommand({
  commandRef: "command.adapter.one",
  idempotencyRef: "idempotency.adapter.one",
  ownerRef: "owner.alice",
  tenantRef: "tenant.acme",
  computerRef: "computer.six",
  workspaceRef: "workspace.six",
  runtimeRef: "runtime.six",
  runtimeGeneration: 4,
  providerLeaseRef: "lease.six",
  capabilityRefs: ["capability.exec"],
  capabilitySnapshotDigest: sha("a"),
  authorityDigest: sha("b"),
  budgetSnapshotDigest: sha("c"),
  budget: { wallTimeMs: 30_000, cpuTimeMs: 20_000, outputBytes: 1_000 },
  kind: "exec",
  argv: ["true"],
  workingDirectory: "/workspace",
  environmentDigest: sha("d"),
  issuedAt: "2026-08-22T12:00:00.000Z",
  timeoutMs: 30_000,
  deadlineAt: "2026-08-22T12:00:30.000Z",
});

describe("Postgres cloud computer dispatch adapter", () => {
  test("maps the durable fence, reservation, and original ACK evidence without synthesis", async () => {
    const calls: Array<Readonly<{ kind: string; input?: unknown }>> = [];
    const stored: StoredCommand = {
      commandRef: command.commandRef,
      requestDigest: command.requestDigest,
      computerRef: command.computerRef,
      workspaceRef: command.workspaceRef,
      sessionRef: "session.six",
      runtimeGeneration: command.runtimeGeneration,
      runtimeRef: command.runtimeRef,
      providerLeaseRef: command.providerLeaseRef,
      status: "not_dispatched",
      providerCommandRef: null,
      dispatchRef: null,
      terminalRef: null,
      terminalDigest: null,
      terminalReason: null,
      terminalSessionSequence: null,
      terminalCommandSequence: null,
      exitCode: null,
      outputDigest: null,
      fence: 1,
      replayed: false,
    };
    const store = {
      get: async () => stored,
      getDispatchAttempt: async () => ({
        dispatchRef: "dispatch.adapter.one",
        status: "prepared" as const,
        runtimeGeneration: 4,
        runtimeRef: "runtime.six",
        providerLeaseRef: "lease.six",
        reservationRef: null,
        providerExecutionRef: null,
        providerCommandRef: null,
        acknowledgementEventRef: null,
        acknowledgementEventDigest: null,
      }),
      prepareDispatchAttempt: async (input: unknown) => {
        calls.push({ kind: "prepare", input });
      },
      exposeDispatchAttempt: async (input: unknown) => {
        calls.push({ kind: "expose", input });
        return { authentic: () => true } as unknown as CloudComputerCommandDispatchClaim;
      },
      markMayHaveStarted: async (input: unknown) => {
        calls.push({ kind: "uncertain", input });
      },
      recordReservation: async (input: unknown) => {
        calls.push({ kind: "reservation", input });
      },
      recordDispatchedAcknowledgement: async (input: unknown) => {
        calls.push({ kind: "acknowledgement", input });
        return stored;
      },
    } as unknown as Pick<
      PostgresCloudComputerCommandStore,
      | "get"
      | "getDispatchAttempt"
      | "prepareDispatchAttempt"
      | "exposeDispatchAttempt"
      | "markMayHaveStarted"
      | "recordReservation"
      | "recordDispatchedAcknowledgement"
    >;
    const durable = createPostgresCloudComputerDurableDispatchPort(store);
    await expect(durable.load(command.commandRef)).resolves.toMatchObject({
      dispatchRef: "dispatch.adapter.one",
      stage: "prepared",
      requestDigest: command.requestDigest,
    });
    const request = {
      command,
      sessionRef: "session.six",
      attachmentEpoch: 3,
      dispatchRef: "dispatch.adapter.one",
      observedAt: "2026-08-22T12:00:01.000Z",
    } as const;
    await durable.prepare(request);
    await durable.expose(request);
    await durable.markUncertain(request);
    const reservation = {
      reservationRef: "reservation.adapter.one",
      providerExecutionRef: "execution.adapter.one",
      reservedAt: "2026-08-22T12:00:02.000Z",
    } as CloudComputerRuntimeReservation;
    const acknowledgement = {
      eventRef: "event.adapter.accepted",
      eventDigest: sha("e"),
      observedAt: "2026-08-22T12:00:03.000Z",
      fence: 1,
      acceptedSequence: 1,
    } as CloudComputerRuntimeAcknowledgement;
    const evidence = {
      ...request,
      providerCommandRef: "provider-command.adapter.one",
      reservation,
      acknowledgement,
    };
    await durable.recordReservation(evidence);
    await durable.recordAcknowledgement(evidence);
    expect(calls.map(({ kind }) => kind)).toEqual([
      "prepare",
      "expose",
      "uncertain",
      "reservation",
      "acknowledgement",
    ]);
    expect(calls.at(-1)?.input).toMatchObject({
      sessionRef: "session.six",
      attachmentEpoch: 3,
      reservationRef: "reservation.adapter.one",
      providerExecutionRef: "execution.adapter.one",
      providerCommandRef: "provider-command.adapter.one",
      acknowledgementEventRef: "event.adapter.accepted",
      acknowledgementEventDigest: sha("e"),
      expectedFence: 1,
      expectedCommandSequence: 1,
      observedAt: "2026-08-22T12:00:03.000Z",
    });
  });

  test("dispatches a recovered prepared command exactly once after controller restart", async () => {
    const runtime = new CloudComputerReverseDialSession({
      sessionRef: "session.six",
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      computerRef: command.computerRef,
      workspaceRef: command.workspaceRef,
      runtimeRef: command.runtimeRef,
      runtimeGeneration: command.runtimeGeneration,
      providerLeaseRef: command.providerLeaseRef,
      authorityDigest: command.authorityDigest,
    });
    runtime.admit(command);
    const credential = await cloudComputerReverseDialCredentialAuthority({
      authorize: async () => true,
    }).issue({
      sessionRef: "session.six",
      ownerRef: command.ownerRef,
      tenantRef: command.tenantRef,
      computerRef: command.computerRef,
      workspaceRef: command.workspaceRef,
      runtimeRef: command.runtimeRef,
      runtimeGeneration: command.runtimeGeneration,
      providerLeaseRef: command.providerLeaseRef,
      nonce: "nonce.adapter.restart",
      authorityDigest: command.authorityDigest,
      issuedAt: "2026-08-22T12:00:00.000Z",
      expiresAt: "2026-08-22T12:00:10.000Z",
    });
    const attached = runtime.attach({
      connectionRef: "connection.adapter.restart",
      credential,
      now: "2026-08-22T12:00:01.000Z",
      cursors: [],
    });
    const reservation = runtime.reserve({
      attachmentEpoch: attached.attachmentEpoch,
      commandRef: command.commandRef,
      requestDigest: command.requestDigest,
      reservationRef: "reservation.adapter.restart",
      providerExecutionRef: "execution.adapter.restart",
      reservedAt: "2026-08-22T12:00:02.000Z",
    }).reservation;
    const acknowledgement = runtime.acknowledge({
      attachmentEpoch: attached.attachmentEpoch,
      commandRef: command.commandRef,
      requestDigest: command.requestDigest,
      reservationRef: reservation.reservationRef,
      providerExecutionRef: reservation.providerExecutionRef,
      eventRef: "event.adapter.restart.accepted",
      sequence: 1,
      observedAt: "2026-08-22T12:00:03.000Z",
    });
    const stored: StoredCommand = {
      commandRef: command.commandRef,
      requestDigest: command.requestDigest,
      computerRef: command.computerRef,
      workspaceRef: command.workspaceRef,
      sessionRef: "session.six",
      runtimeGeneration: command.runtimeGeneration,
      runtimeRef: command.runtimeRef,
      providerLeaseRef: command.providerLeaseRef,
      status: "not_dispatched",
      providerCommandRef: null,
      dispatchRef: "dispatch.adapter.restart",
      terminalRef: null,
      terminalDigest: null,
      terminalReason: null,
      exitCode: null,
      outputDigest: null,
      terminalSessionSequence: null,
      terminalCommandSequence: null,
      fence: 1,
      replayed: false,
    };
    const attempt = {
      dispatchRef: "dispatch.adapter.restart",
      status: "prepared" as "prepared" | "write_exposed" | "reservation_recorded" | "acknowledged",
      runtimeGeneration: command.runtimeGeneration,
      runtimeRef: command.runtimeRef,
      providerLeaseRef: command.providerLeaseRef,
      reservationRef: null as string | null,
      providerExecutionRef: null as string | null,
      providerCommandRef: null as string | null,
      acknowledgementEventRef: null as string | null,
      acknowledgementEventDigest: null as string | null,
    };
    const store = {
      loadCommandForDispatch: async () => ({
        command,
        requestDigest: command.requestDigest,
        dispatchRef: attempt.dispatchRef,
      }),
      get: async () => stored,
      getDispatchAttempt: async () => ({ ...attempt }),
      prepareDispatchAttempt: async () => {
        throw new Error("prepared attempt must be reused");
      },
      exposeDispatchAttempt: async () => {
        attempt.status = "write_exposed";
        return { authentic: () => true } as unknown as CloudComputerCommandDispatchClaim;
      },
      markMayHaveStarted: async () => undefined,
      recordReservation: async (input: {
        reservationRef: string;
        providerExecutionRef: string;
        providerCommandRef: string;
      }) => {
        attempt.status = "reservation_recorded";
        attempt.reservationRef = input.reservationRef;
        attempt.providerExecutionRef = input.providerExecutionRef;
        attempt.providerCommandRef = input.providerCommandRef;
      },
      recordDispatchedAcknowledgement: async (input: {
        acknowledgementEventRef: string;
        acknowledgementEventDigest: string;
      }) => {
        attempt.status = "acknowledged";
        attempt.acknowledgementEventRef = input.acknowledgementEventRef;
        attempt.acknowledgementEventDigest = input.acknowledgementEventDigest;
        return { ...stored, status: "dispatched" as const };
      },
    };
    const recovered = await store.loadCommandForDispatch();
    const durable = createPostgresCloudComputerDurableDispatchPort(
      store as unknown as PostgresCloudComputerCommandStore,
    );
    let writes = 0;
    const dispatcher = createCloudComputerCommandDispatcher({
      durable,
      transport: {
        write: async () => {
          writes += 1;
          return {
            providerCommandRef: "provider-command.adapter.restart",
            reservation,
            acknowledgement,
          };
        },
      },
    });
    const request = {
      command: recovered.command,
      sessionRef: "session.six",
      attachmentEpoch: attached.attachmentEpoch,
      dispatchRef: recovered.dispatchRef!,
      observedAt: "2026-08-22T12:00:01.000Z",
    };
    await expect(dispatcher.dispatch(request)).resolves.toEqual({ outcome: "dispatched" });
    await expect(dispatcher.dispatch(request)).resolves.toEqual({ outcome: "dispatched" });
    expect(writes).toBe(1);
  });
});
