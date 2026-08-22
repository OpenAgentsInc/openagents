import { describe, expect, test } from "vite-plus/test";
import {
  CloudComputerReverseDialSession,
  assertCloudComputerCommand,
  cloudComputerReverseDialCredentialAuthority,
  createCloudComputerCommand,
  createCloudComputerCommandEvent,
  type CloudComputerCommand,
  type CloudComputerCommandEventKind,
  type CloudComputerCommandCursor,
} from "./cloud-computer-command.js";
import { cloudComputerCommandRecoveryEvidenceAuthority } from "./cloud-computer-command-recovery.js";

const at = (ms: number) => new Date(Date.UTC(2026, 7, 22, 12, 0, 0, ms)).toISOString();
const sha = (value: string): `sha256:${string}` => `sha256:${value.repeat(64)}`;
const makeCommand = (overrides: Partial<CloudComputerCommand> = {}) =>
  createCloudComputerCommand({
    commandRef: "command.six.one",
    idempotencyRef: "idempotency.six.one",
    ownerRef: "owner.alice",
    tenantRef: "tenant.acme",
    computerRef: "computer.six",
    workspaceRef: "workspace.six",
    runtimeRef: "runtime.six",
    runtimeGeneration: 4,
    providerLeaseRef: "lease.six",
    capabilityRefs: ["capability.exec", "capability.git"],
    capabilitySnapshotDigest: sha("a"),
    authorityDigest: sha("b"),
    budgetSnapshotDigest: sha("c"),
    budget: { wallTimeMs: 30_000, cpuTimeMs: 20_000, outputBytes: 1_000_000 },
    kind: "exec",
    argv: ["git", "status"],
    workingDirectory: "/workspace",
    environmentDigest: sha("d"),
    issuedAt: at(0),
    timeoutMs: 30_000,
    deadlineAt: at(30_000),
    ...overrides,
  });
const makeSession = () =>
  new CloudComputerReverseDialSession({
    sessionRef: "session.six",
    ownerRef: "owner.alice",
    tenantRef: "tenant.acme",
    computerRef: "computer.six",
    workspaceRef: "workspace.six",
    runtimeRef: "runtime.six",
    runtimeGeneration: 4,
    providerLeaseRef: "lease.six",
    authorityDigest: sha("b"),
  });
const authority = cloudComputerReverseDialCredentialAuthority({ authorize: async () => true });
let nonce = 0;
const credential = () =>
  authority.issue({
    sessionRef: "session.six",
    ownerRef: "owner.alice",
    tenantRef: "tenant.acme",
    computerRef: "computer.six",
    workspaceRef: "workspace.six",
    runtimeRef: "runtime.six",
    runtimeGeneration: 4,
    providerLeaseRef: "lease.six",
    nonce: `nonce.six.${++nonce}`,
    authorityDigest: sha("b"),
    issuedAt: at(0),
    expiresAt: at(10_000),
  });
const attach = async (
  session: CloudComputerReverseDialSession,
  cursors: ReadonlyArray<CloudComputerCommandCursor> = [],
) =>
  session.attach({
    connectionRef: `connection.six.${nonce + 1}`,
    credential: await credential(),
    now: at(1_000),
    cursors,
  });
const reserveAck = async () => {
  const session = makeSession();
  const command = makeCommand();
  session.admit(command);
  const connected = await attach(session);
  const reservation = session.reserve({
    attachmentEpoch: connected.attachmentEpoch,
    commandRef: command.commandRef,
    requestDigest: command.requestDigest,
    reservationRef: "reservation.six.one",
    providerExecutionRef: "execution.six.one",
    reservedAt: at(1_100),
  });
  const acknowledgement = session.acknowledge({
    attachmentEpoch: connected.attachmentEpoch,
    commandRef: command.commandRef,
    requestDigest: command.requestDigest,
    reservationRef: reservation.reservation.reservationRef,
    providerExecutionRef: reservation.reservation.providerExecutionRef,
    eventRef: "event.six.accepted",
    sequence: 1,
    observedAt: at(1_200),
  });
  return { session, command, connected, reservation, acknowledgement };
};
const event = (
  command: CloudComputerCommand,
  sequence: number,
  fence: number,
  kind: CloudComputerCommandEventKind,
  payload: Readonly<Record<string, unknown>> = {},
  eventRef = `event.six.${sequence}`,
) =>
  createCloudComputerCommandEvent({
    eventRef,
    commandRef: command.commandRef,
    requestDigest: command.requestDigest,
    providerExecutionRef: "execution.six.one",
    sessionRef: "session.six",
    runtimeRef: "runtime.six",
    runtimeGeneration: 4,
    sequence,
    fence,
    kind,
    payload,
    observedAt: at(1_200 + sequence),
  });

describe("cloud computer command protocol", () => {
  test("binds idempotency, authority, capabilities, budgets, and secret-free environment metadata", () => {
    const session = makeSession();
    const command = makeCommand();
    expect(Object.keys(command)).not.toContain("environment");
    expect(session.admit(command).replayed).toBe(false);
    expect(session.admit(makeCommand()).replayed).toBe(true);
    expect(() => session.admit(makeCommand({ argv: ["git", "diff"] }))).toThrowError(
      /idempotencyRef/u,
    );
    expect(() =>
      makeCommand({ capabilityRefs: ["capability.git", "capability.exec"] }),
    ).toThrowError(/capabilityRefs/u);
  });

  test("verifies exact reconstructed command bytes and rejects raw environment material", () => {
    const command = makeCommand();
    expect(() => assertCloudComputerCommand(command)).not.toThrow();
    expect(() => assertCloudComputerCommand({ ...command, requestDigest: sha("f") })).toThrowError(
      /requestDigest/u,
    );
    expect(() =>
      assertCloudComputerCommand({ ...command, environment: { TOKEN: "secret" } }),
    ).toThrowError(/invalid: command/u);
    expect(() =>
      assertCloudComputerCommand({
        ...command,
        budget: { ...command.budget, outputBytes: "large" },
      }),
    ).toThrowError(/command.budget/u);
  });

  test("requires an authority-issued short-lived nonce and monotonically fences attachments", async () => {
    const session = makeSession();
    session.admit(makeCommand());
    const token = await credential();
    const first = session.attach({
      connectionRef: "connection.six.first",
      credential: token,
      now: at(1_000),
      cursors: [],
    });
    expect(() =>
      session.attach({
        connectionRef: "connection.six.replay",
        credential: token,
        now: at(1_001),
        cursors: [],
      }),
    ).toThrowError(/credential/u);
    const second = await attach(session);
    expect(second.attachmentEpoch).toBe(first.attachmentEpoch + 1);
    expect(() =>
      session.reserve({
        attachmentEpoch: first.attachmentEpoch,
        commandRef: makeCommand().commandRef,
        requestDigest: makeCommand().requestDigest,
        reservationRef: "reservation.stale",
        providerExecutionRef: "execution.stale",
        reservedAt: at(1_100),
      }),
    ).toThrowError(/stale_attachment/u);
  });

  test("persists a provider execution reservation before ACK and replays both exactly", async () => {
    const { session, command, connected, reservation, acknowledgement } = await reserveAck();
    expect(reservation).toMatchObject({
      replayed: false,
      reservation: { providerExecutionRef: "execution.six.one" },
    });
    expect(acknowledgement).toMatchObject({
      acceptedSequence: 1,
      providerExecutionRef: "execution.six.one",
      replayed: false,
    });
    expect(
      session.reserve({
        attachmentEpoch: connected.attachmentEpoch,
        commandRef: command.commandRef,
        requestDigest: command.requestDigest,
        reservationRef: "reservation.six.one",
        providerExecutionRef: "execution.six.one",
        reservedAt: at(1_100),
      }).replayed,
    ).toBe(true);
    expect(
      session.acknowledge({
        attachmentEpoch: connected.attachmentEpoch,
        commandRef: command.commandRef,
        requestDigest: command.requestDigest,
        reservationRef: "reservation.six.one",
        providerExecutionRef: "execution.six.one",
        eventRef: "event.six.accepted",
        sequence: 1,
        observedAt: at(1_200),
      }).replayed,
    ).toBe(true);
  });

  test("sequences and deduplicates events per command execution and resumes its cursor", async () => {
    const { session, command, connected } = await reserveAck();
    const tool = event(command, 2, 1, "tool", { toolRef: "tool.git" });
    const checkpoint = event(command, 3, 1, "checkpoint", { checkpointRef: "checkpoint.six" });
    expect(
      session.append({ attachmentEpoch: connected.attachmentEpoch, event: tool }).replayed,
    ).toBe(false);
    expect(
      session.append({ attachmentEpoch: connected.attachmentEpoch, event: tool }).replayed,
    ).toBe(true);
    expect(() =>
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: { ...tool, payload: { toolRef: "tool.changed" } },
      }),
    ).toThrowError(/eventDigest/u);
    expect(() =>
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: event(command, 2, 1, "lifecycle", {}, "event.six.conflict"),
      }),
    ).toThrowError(/cursor_regressed/u);
    session.append({ attachmentEpoch: connected.attachmentEpoch, event: checkpoint });
    const cursor = { ...session.cursor(command.commandRef), nextSequence: 3 };
    const resumed = await attach(session, [cursor]);
    expect(resumed.executions[0]?.events).toEqual([checkpoint]);
    expect(resumed.executions[0]?.cursor?.nextSequence).toBe(4);
  });

  test("binds cursors to execution and retention epoch", async () => {
    const { session, command, connected } = await reserveAck();
    session.append({
      attachmentEpoch: connected.attachmentEpoch,
      event: event(command, 2, 1, "stdout", { bytes: "ok" }),
    });
    const old = session.cursor(command.commandRef);
    session.compactThrough(command.commandRef, 1);
    await expect(attach(session, [old])).rejects.toThrowError(/cursor_expired/u);
    const future = { ...session.cursor(command.commandRef), nextSequence: 99 };
    await expect(attach(session, [future])).rejects.toThrowError(/cursor_regressed/u);
  });

  test("fences cancellation and records immutable terminal evidence", async () => {
    const { session, command, connected } = await reserveAck();
    const cancelled = session.requestCancellation({
      cancellationRef: "cancellation.six.one",
      commandRef: command.commandRef,
      requestDigest: command.requestDigest,
      expectedFence: 1,
      reason: "user_requested",
      requestedAt: at(2_000),
    });
    expect(cancelled.fence).toBe(2);
    expect(() =>
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: event(command, 2, 1, "stdout"),
      }),
    ).toThrowError(/stale_fence/u);
    const terminalEvent = event(command, 2, 2, "cancelled", {
      exitCode: null,
      outputDigest: null,
      reason: "user_requested",
    });
    const terminal = session.append({
      attachmentEpoch: connected.attachmentEpoch,
      event: terminalEvent,
    }).terminal;
    expect(terminal).toMatchObject({
      outcome: "cancelled",
      fence: 2,
      eventDigest: terminalEvent.eventDigest,
    });
    expect(terminal?.evidenceDigest).toMatch(/^sha256:/u);
  });

  test("fails closed on late events, invalid terminal evidence, and unverified loss", async () => {
    const { session, command, connected } = await reserveAck();
    expect(() =>
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: event(command, 2, 1, "failed", {
          exitCode: 1,
          outputDigest: "bad",
          reason: "failed",
        }),
      }),
    ).toThrowError(/terminal.payload/u);
    expect(() =>
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: event(command, 2, 1, "failed", {
          exitCode: 1,
          outputDigest: null,
          reason: "Process failed",
        }),
      }),
    ).toThrowError(/terminal.payload/u);
    const rawLost = event(
      command,
      2,
      1,
      "lost",
      { exitCode: null, outputDigest: null, reason: "provider_lost" },
      "event.six.lost",
    );
    expect(() =>
      session.append({ attachmentEpoch: connected.attachmentEpoch, event: rawLost }),
    ).toThrowError(/lossEvidence.authority/u);
    const lossAuthority = cloudComputerCommandRecoveryEvidenceAuthority({
      verify: async () => true,
    });
    const lossEvidence = await lossAuthority.issue({
      kind: "runtime_lost",
      evidenceRef: "evidence.runtime.lost",
      evidenceDigest: sha("e"),
      computerRef: command.computerRef,
      workspaceRef: command.workspaceRef,
      runtimeRef: command.runtimeRef,
      runtimeGeneration: command.runtimeGeneration,
      providerLeaseRef: command.providerLeaseRef,
      observedAt: at(1_201),
      durable: true,
    });
    const lost = event(
      command,
      2,
      1,
      "lost",
      {
        exitCode: null,
        outputDigest: null,
        reason: "provider_lost",
        lossEvidenceRef: lossEvidence.evidenceRef,
        lossEvidenceDigest: lossEvidence.evidenceDigest,
      },
      "event.six.lost",
    );
    expect(
      session.append({
        attachmentEpoch: connected.attachmentEpoch,
        event: lost,
        recoveryEvidence: lossEvidence,
      }).terminal,
    ).toMatchObject({ outcome: "lost", providerExecutionRef: "execution.six.one" });
    const other = await reserveAck();
    const late = createCloudComputerCommandEvent({
      ...event(other.command, 2, 1, "stdout"),
      eventRef: "event.six.late",
      observedAt: at(31_000),
    });
    expect(() =>
      other.session.append({ attachmentEpoch: other.connected.attachmentEpoch, event: late }),
    ).toThrowError(/deadline_expired/u);
    expect(() =>
      other.session.timeout({
        commandRef: other.command.commandRef,
        requestDigest: other.command.requestDigest,
        expectedFence: 1,
        eventRef: "event.six.timeout",
        observedAt: at(29_999),
        reason: "deadline_exceeded",
      }),
    ).toThrowError(/timeoutBeforeDeadline/u);
    const timedOut = other.session.timeout({
      commandRef: other.command.commandRef,
      requestDigest: other.command.requestDigest,
      expectedFence: 1,
      eventRef: "event.six.timeout",
      observedAt: at(30_000),
      reason: "deadline_exceeded",
    });
    expect(timedOut).toMatchObject({ outcome: "timed_out", fence: 2, sequence: 2 });
  });
});
