import { describe, expect, test } from "vite-plus/test";
import {
  CloudComputerReverseDialSession,
  cloudComputerReverseDialCredentialAuthority,
  createCloudComputerCommand,
  type CloudComputerCommand,
} from "./cloud-computer-command.js";
import {
  createCloudComputerCommandDispatcher,
  type CloudComputerDurableDispatchPort,
  type CloudComputerDurableDispatchState,
} from "./cloud-computer-command-dispatch.js";

const sha = (value: string): `sha256:${string}` => `sha256:${value.repeat(64)}`;
const issuedAt = "2026-08-22T12:00:00.000Z";
const command = createCloudComputerCommand({
  commandRef: "command.dispatch.one",
  idempotencyRef: "idempotency.dispatch.one",
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
  issuedAt,
  timeoutMs: 30_000,
  deadlineAt: "2026-08-22T12:00:30.000Z",
});

const runtimeEvidence = async (input: CloudComputerCommand = command) => {
  const session = new CloudComputerReverseDialSession({
    sessionRef: "session.six",
    ownerRef: input.ownerRef,
    tenantRef: input.tenantRef,
    computerRef: input.computerRef,
    workspaceRef: input.workspaceRef,
    runtimeRef: input.runtimeRef,
    runtimeGeneration: input.runtimeGeneration,
    providerLeaseRef: input.providerLeaseRef,
    authorityDigest: input.authorityDigest,
  });
  session.admit(input);
  const credential = await cloudComputerReverseDialCredentialAuthority({
    authorize: async () => true,
  }).issue({
    sessionRef: "session.six",
    ownerRef: input.ownerRef,
    tenantRef: input.tenantRef,
    computerRef: input.computerRef,
    workspaceRef: input.workspaceRef,
    runtimeRef: input.runtimeRef,
    runtimeGeneration: input.runtimeGeneration,
    providerLeaseRef: input.providerLeaseRef,
    nonce: "nonce.dispatch.one",
    authorityDigest: input.authorityDigest,
    issuedAt,
    expiresAt: "2026-08-22T12:00:10.000Z",
  });
  const attached = session.attach({
    connectionRef: "connection.dispatch.one",
    credential,
    now: "2026-08-22T12:00:01.000Z",
    cursors: [],
  });
  const reservation = session.reserve({
    attachmentEpoch: attached.attachmentEpoch,
    commandRef: input.commandRef,
    requestDigest: input.requestDigest,
    reservationRef: "reservation.dispatch.one",
    providerExecutionRef: "execution.dispatch.one",
    reservedAt: "2026-08-22T12:00:02.000Z",
  }).reservation;
  const acknowledgement = session.acknowledge({
    attachmentEpoch: attached.attachmentEpoch,
    commandRef: input.commandRef,
    requestDigest: input.requestDigest,
    reservationRef: reservation.reservationRef,
    providerExecutionRef: reservation.providerExecutionRef,
    eventRef: "event.dispatch.accepted",
    sequence: 1,
    observedAt: "2026-08-22T12:00:03.000Z",
  });
  return { providerCommandRef: "provider-command.dispatch.one", reservation, acknowledgement };
};

class MemoryDurable implements CloudComputerDurableDispatchPort<Readonly<{ dispatchRef: string }>> {
  state: CloudComputerDurableDispatchState | null = null;
  readonly events: string[] = [];

  async load() {
    return this.state;
  }
  async prepare(input: { command: CloudComputerCommand; dispatchRef: string }) {
    this.events.push("prepare");
    this.state = {
      commandRef: input.command.commandRef,
      requestDigest: input.command.requestDigest,
      dispatchRef: input.dispatchRef,
      stage: "prepared",
      providerCommandRef: null,
      reservationRef: null,
      providerExecutionRef: null,
      acknowledgementEventRef: null,
      acknowledgementEventDigest: null,
      reservation: null,
      acknowledgement: null,
    };
  }
  async expose(input: { dispatchRef: string }) {
    this.events.push("expose");
    this.state = { ...this.state!, stage: "exposed" };
    return { dispatchRef: input.dispatchRef };
  }
  async markUncertain() {
    this.events.push("uncertain");
  }
  async recordReservation(input: {
    providerCommandRef: string;
    reservation: CloudComputerDurableDispatchState["reservation"];
  }) {
    this.events.push("reservation");
    this.state = {
      ...this.state!,
      stage: "reservation_recorded",
      providerCommandRef: input.providerCommandRef,
      reservationRef: input.reservation?.reservationRef ?? null,
      providerExecutionRef: input.reservation?.providerExecutionRef ?? null,
      reservation: input.reservation,
    };
  }
  async recordAcknowledgement(input: {
    reservation: CloudComputerDurableDispatchState["reservation"];
    acknowledgement: CloudComputerDurableDispatchState["acknowledgement"];
  }) {
    this.events.push("acknowledgement");
    this.state = {
      ...this.state!,
      stage: "acknowledged",
      acknowledgementEventRef: input.acknowledgement?.eventRef ?? null,
      acknowledgementEventDigest: input.acknowledgement?.eventDigest ?? null,
      reservation: input.reservation,
      acknowledgement: input.acknowledgement,
    };
  }
}

const request = {
  command,
  sessionRef: "session.six",
  attachmentEpoch: 1,
  dispatchRef: "dispatch.one",
  observedAt: issuedAt,
} as const;

describe("cloud computer durable dispatch coordinator", () => {
  test("persists exposure before one transport write and reservation before ACK", async () => {
    const durable = new MemoryDurable();
    const runtime = await runtimeEvidence();
    const dispatcher = createCloudComputerCommandDispatcher({
      durable,
      transport: {
        write: async ({ claim }) => {
          durable.events.push(`transport:${claim.dispatchRef}`);
          return runtime;
        },
      },
    });
    await expect(dispatcher.dispatch(request)).resolves.toEqual({ outcome: "dispatched" });
    expect(durable.events).toEqual([
      "prepare",
      "expose",
      "transport:dispatch.one",
      "reservation",
      "acknowledgement",
    ]);
  });

  test("retries a crash before exposure but never retries after exposure", async () => {
    const durable = new MemoryDurable();
    const runtime = await runtimeEvidence();
    let crash = true;
    let writes = 0;
    const dispatcher = createCloudComputerCommandDispatcher({
      durable,
      transport: {
        write: async () => {
          writes += 1;
          return runtime;
        },
      },
      faults: {
        afterPrepare: () => {
          if (crash) throw new Error("crash-before-exposure");
        },
      },
    });
    await expect(dispatcher.dispatch(request)).rejects.toThrowError("crash-before-exposure");
    crash = false;
    await expect(dispatcher.dispatch(request)).resolves.toEqual({ outcome: "dispatched" });
    expect(writes).toBe(1);

    const exposed = new MemoryDurable();
    const crashing = createCloudComputerCommandDispatcher({
      durable: exposed,
      transport: {
        write: async () => {
          writes += 1;
          return runtime;
        },
      },
      faults: {
        afterExposure: () => {
          throw new Error("crash-after-exposure");
        },
      },
    });
    await expect(crashing.dispatch(request)).rejects.toThrowError("crash-after-exposure");
    await expect(crashing.dispatch(request)).resolves.toEqual({ outcome: "observation_required" });
    expect(writes).toBe(1);
  });

  test("does not rewrite after a lost response and completes a persisted reservation by reattachment", async () => {
    const runtime = await runtimeEvidence();
    const lost = new MemoryDurable();
    let writes = 0;
    const lostDispatcher = createCloudComputerCommandDispatcher({
      durable: lost,
      transport: {
        write: async () => {
          writes += 1;
          throw new Error("lost-response");
        },
      },
    });
    await expect(lostDispatcher.dispatch(request)).rejects.toThrowError("lost-response");
    await expect(lostDispatcher.dispatch(request)).resolves.toEqual({
      outcome: "observation_required",
    });
    expect(writes).toBe(1);
    expect(lost.events).toContain("uncertain");

    const durable = new MemoryDurable();
    let crash = true;
    const dispatcher = createCloudComputerCommandDispatcher({
      durable,
      transport: { write: async () => runtime },
      faults: {
        afterReservation: () => {
          if (crash) throw new Error("crash-after-reservation");
        },
      },
    });
    await expect(dispatcher.dispatch(request)).rejects.toThrowError("crash-after-reservation");
    crash = false;
    await expect(dispatcher.dispatch(request)).resolves.toEqual({
      outcome: "observation_required",
    });
    await dispatcher.acceptRuntimeAcknowledgement({ ...request, ...runtime });
    expect(durable.state).toMatchObject({
      stage: "acknowledged",
      reservation: { reservationRef: "reservation.dispatch.one" },
      acknowledgement: { eventRef: "event.dispatch.accepted" },
    });
    await expect(
      dispatcher.acceptRuntimeAcknowledgement({ ...request, ...runtime }),
    ).resolves.toBeUndefined();
    await expect(
      dispatcher.acceptRuntimeAcknowledgement({
        ...request,
        ...runtime,
        providerCommandRef: "provider-command.changed",
      }),
    ).rejects.toThrowError(/dispatchAcknowledgement/u);
    await expect(
      dispatcher.acceptRuntimeAcknowledgement({
        ...request,
        ...runtime,
        dispatchRef: "dispatch.changed",
      }),
    ).rejects.toThrowError(/dispatchAcknowledgement/u);
    const changedCommand = createCloudComputerCommand({
      ...command,
      argv: ["false"],
    });
    await expect(
      dispatcher.acceptRuntimeAcknowledgement({
        ...request,
        ...runtime,
        command: changedCommand,
      }),
    ).rejects.toThrowError(/dispatchMapping/u);
  });

  test("rejects structurally forged runtime reservation evidence after exposure", async () => {
    const durable = new MemoryDurable();
    const runtime = await runtimeEvidence();
    const dispatcher = createCloudComputerCommandDispatcher({
      durable,
      transport: {
        write: async () => ({ ...runtime, reservation: { ...runtime.reservation } }),
      },
    });
    await expect(dispatcher.dispatch(request)).rejects.toThrowError(/runtimeReservation/u);
    expect(durable.state?.stage).toBe("exposed");
    await expect(dispatcher.dispatch(request)).resolves.toEqual({
      outcome: "observation_required",
    });
  });
});
