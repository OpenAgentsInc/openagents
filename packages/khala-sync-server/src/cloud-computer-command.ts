import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";

import {
  assertCloudComputerCommandRecoveryEvidence,
  type CloudComputerCommandRecoveryEvidence,
} from "./cloud-computer-command-recovery.js";

export const CLOUD_COMPUTER_COMMAND_SCHEMA = "openagents.cloud_computer_command.v1" as const;
export const CLOUD_COMPUTER_COMMAND_EVENT_SCHEMA =
  "openagents.cloud_computer_command_event.v1" as const;
export const CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA =
  "openagents.cloud_computer_command_cursor.v1" as const;
export const CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA =
  "openagents.cloud_computer_command_terminal.v1" as const;
export const CLOUD_COMPUTER_RUNTIME_RESERVATION_SCHEMA =
  "openagents.cloud_computer_runtime_reservation.v1" as const;
export type CommandDigest = `sha256:${string}`;
const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const hash = (value: unknown): CommandDigest =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;

export class CloudComputerCommandError extends Error {
  override readonly name = "CloudComputerCommandError";
  constructor(
    readonly code:
      | "conflict"
      | "cursor_expired"
      | "cursor_regressed"
      | "deadline_expired"
      | "invalid"
      | "not_found"
      | "scope_mismatch"
      | "stale_attachment"
      | "stale_fence"
      | "terminal",
    readonly field: string,
  ) {
    super(`${code}: ${field}`);
  }
}
const ref = (value: string, field: string) => {
  if (!REF.test(value)) throw new CloudComputerCommandError("invalid", field);
};
const digest = (value: string, field: string) => {
  if (!DIGEST.test(value)) throw new CloudComputerCommandError("invalid", field);
};
const time = (value: string, field: string) => {
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) throw new CloudComputerCommandError("invalid", field);
  return parsed;
};
const integer = (value: number, field: string, minimum = 0) => {
  if (!Number.isSafeInteger(value) || value < minimum)
    throw new CloudComputerCommandError("invalid", field);
};

export type CloudComputerCommandInput = Readonly<{
  commandRef: string;
  idempotencyRef: string;
  ownerRef: string;
  tenantRef: string;
  computerRef: string;
  workspaceRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  providerLeaseRef: string;
  capabilityRefs: ReadonlyArray<string>;
  capabilitySnapshotDigest: CommandDigest;
  authorityDigest: CommandDigest;
  budgetSnapshotDigest: CommandDigest;
  budget: Readonly<{ wallTimeMs: number; cpuTimeMs: number; outputBytes: number }>;
  kind: "exec";
  argv: ReadonlyArray<string>;
  workingDirectory: string;
  environmentDigest: CommandDigest;
  issuedAt: string;
  timeoutMs: number;
  deadlineAt: string;
}>;
const commandBytes = (input: CloudComputerCommandInput) => ({
  schema: CLOUD_COMPUTER_COMMAND_SCHEMA,
  commandRef: input.commandRef,
  idempotencyRef: input.idempotencyRef,
  ownerRef: input.ownerRef,
  tenantRef: input.tenantRef,
  computerRef: input.computerRef,
  workspaceRef: input.workspaceRef,
  runtimeRef: input.runtimeRef,
  runtimeGeneration: input.runtimeGeneration,
  providerLeaseRef: input.providerLeaseRef,
  capabilityRefs: [...input.capabilityRefs],
  capabilitySnapshotDigest: input.capabilitySnapshotDigest,
  authorityDigest: input.authorityDigest,
  budgetSnapshotDigest: input.budgetSnapshotDigest,
  budget: { ...input.budget },
  kind: input.kind,
  argv: [...input.argv],
  workingDirectory: input.workingDirectory,
  environmentDigest: input.environmentDigest,
  issuedAt: input.issuedAt,
  timeoutMs: input.timeoutMs,
  deadlineAt: input.deadlineAt,
});
export type CloudComputerCommand = ReturnType<typeof commandBytes> &
  Readonly<{ requestDigest: CommandDigest }>;
export const createCloudComputerCommand = (
  input: CloudComputerCommandInput,
): CloudComputerCommand => {
  for (const [field, value] of Object.entries({
    commandRef: input.commandRef,
    idempotencyRef: input.idempotencyRef,
    ownerRef: input.ownerRef,
    tenantRef: input.tenantRef,
    computerRef: input.computerRef,
    workspaceRef: input.workspaceRef,
    runtimeRef: input.runtimeRef,
    providerLeaseRef: input.providerLeaseRef,
  }))
    ref(value, field);
  for (const [field, value] of Object.entries({
    capabilitySnapshotDigest: input.capabilitySnapshotDigest,
    authorityDigest: input.authorityDigest,
    budgetSnapshotDigest: input.budgetSnapshotDigest,
    environmentDigest: input.environmentDigest,
  }))
    digest(value, field);
  integer(input.runtimeGeneration, "runtimeGeneration", 1);
  integer(input.timeoutMs, "timeoutMs", 1);
  integer(input.budget.wallTimeMs, "budget.wallTimeMs", 1);
  integer(input.budget.cpuTimeMs, "budget.cpuTimeMs", 1);
  integer(input.budget.outputBytes, "budget.outputBytes", 1);
  const capabilities = input.capabilityRefs;
  if (
    new Set(capabilities).size !== capabilities.length ||
    capabilities.some((value, index) => index > 0 && value <= capabilities[index - 1]!)
  )
    throw new CloudComputerCommandError("invalid", "capabilityRefs");
  capabilities.forEach((value) => ref(value, "capabilityRefs"));
  if (
    input.kind !== "exec" ||
    input.argv.length === 0 ||
    input.argv.some((value) => value.includes("\0")) ||
    !input.workingDirectory.startsWith("/") ||
    input.workingDirectory.includes("\0")
  )
    throw new CloudComputerCommandError("invalid", "commandPayload");
  const issuedAt = time(input.issuedAt, "issuedAt");
  if (
    time(input.deadlineAt, "deadlineAt") - issuedAt !== input.timeoutMs ||
    input.budget.wallTimeMs > input.timeoutMs
  )
    throw new CloudComputerCommandError("invalid", "deadlineAt");
  const bytes = commandBytes(input);
  return Object.freeze({ ...bytes, requestDigest: hash(bytes) });
};

const commandFields = new Set([
  "schema",
  "commandRef",
  "idempotencyRef",
  "ownerRef",
  "tenantRef",
  "computerRef",
  "workspaceRef",
  "runtimeRef",
  "runtimeGeneration",
  "providerLeaseRef",
  "capabilityRefs",
  "capabilitySnapshotDigest",
  "authorityDigest",
  "budgetSnapshotDigest",
  "budget",
  "kind",
  "argv",
  "workingDirectory",
  "environmentDigest",
  "issuedAt",
  "timeoutMs",
  "deadlineAt",
  "requestDigest",
]);

export function assertCloudComputerCommand(value: unknown): asserts value is CloudComputerCommand {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    Object.keys(value).some((field) => !commandFields.has(field))
  ) {
    throw new CloudComputerCommandError("invalid", "command");
  }
  const command = value as Readonly<Record<string, unknown>>;
  const strings = [
    "commandRef",
    "idempotencyRef",
    "ownerRef",
    "tenantRef",
    "computerRef",
    "workspaceRef",
    "runtimeRef",
    "providerLeaseRef",
    "capabilitySnapshotDigest",
    "authorityDigest",
    "budgetSnapshotDigest",
    "workingDirectory",
    "environmentDigest",
    "issuedAt",
    "deadlineAt",
    "requestDigest",
  ];
  if (
    command.schema !== CLOUD_COMPUTER_COMMAND_SCHEMA ||
    strings.some((field) => typeof command[field] !== "string") ||
    typeof command.runtimeGeneration !== "number" ||
    typeof command.timeoutMs !== "number" ||
    command.kind !== "exec" ||
    !Array.isArray(command.capabilityRefs) ||
    command.capabilityRefs.some((item) => typeof item !== "string") ||
    !Array.isArray(command.argv) ||
    command.argv.some((item) => typeof item !== "string") ||
    typeof command.budget !== "object" ||
    command.budget === null ||
    Array.isArray(command.budget)
  ) {
    throw new CloudComputerCommandError("invalid", "command");
  }
  const budget = command.budget as Readonly<Record<string, unknown>>;
  if (
    Object.keys(budget).some(
      (field) => !["wallTimeMs", "cpuTimeMs", "outputBytes"].includes(field),
    ) ||
    typeof budget.wallTimeMs !== "number" ||
    typeof budget.cpuTimeMs !== "number" ||
    typeof budget.outputBytes !== "number"
  ) {
    throw new CloudComputerCommandError("invalid", "command.budget");
  }
  const rebuilt = createCloudComputerCommand({
    commandRef: command.commandRef as string,
    idempotencyRef: command.idempotencyRef as string,
    ownerRef: command.ownerRef as string,
    tenantRef: command.tenantRef as string,
    computerRef: command.computerRef as string,
    workspaceRef: command.workspaceRef as string,
    runtimeRef: command.runtimeRef as string,
    runtimeGeneration: command.runtimeGeneration,
    providerLeaseRef: command.providerLeaseRef as string,
    capabilityRefs: command.capabilityRefs as ReadonlyArray<string>,
    capabilitySnapshotDigest: command.capabilitySnapshotDigest as CommandDigest,
    authorityDigest: command.authorityDigest as CommandDigest,
    budgetSnapshotDigest: command.budgetSnapshotDigest as CommandDigest,
    budget: {
      wallTimeMs: budget.wallTimeMs,
      cpuTimeMs: budget.cpuTimeMs,
      outputBytes: budget.outputBytes,
    },
    kind: "exec",
    argv: command.argv as ReadonlyArray<string>,
    workingDirectory: command.workingDirectory as string,
    environmentDigest: command.environmentDigest as CommandDigest,
    issuedAt: command.issuedAt as string,
    timeoutMs: command.timeoutMs,
    deadlineAt: command.deadlineAt as string,
  });
  if (rebuilt.requestDigest !== command.requestDigest) {
    throw new CloudComputerCommandError("conflict", "requestDigest");
  }
}

declare const credentialBrand: unique symbol;
export type CloudComputerReverseDialCredential = Readonly<{
  sessionRef: string;
  ownerRef: string;
  tenantRef: string;
  computerRef: string;
  workspaceRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  providerLeaseRef: string;
  nonce: string;
  authorityDigest: CommandDigest;
  issuedAt: string;
  expiresAt: string;
  [credentialBrand]: true;
}>;
type CredentialInput = Omit<CloudComputerReverseDialCredential, typeof credentialBrand>;
const credentials = new WeakSet<object>();
export function assertCloudComputerReverseDialCredential(
  value: unknown,
): asserts value is CloudComputerReverseDialCredential {
  if (typeof value !== "object" || value === null || !credentials.has(value)) {
    throw new CloudComputerCommandError("scope_mismatch", "credential.authority");
  }
}
export const cloudComputerReverseDialCredentialAuthority = (
  adapter: Readonly<{ authorize: (input: CredentialInput) => Promise<boolean> }>,
  maxTtlMs = 60_000,
) => ({
  issue: async (input: CredentialInput): Promise<CloudComputerReverseDialCredential> => {
    for (const [field, value] of Object.entries({
      sessionRef: input.sessionRef,
      ownerRef: input.ownerRef,
      tenantRef: input.tenantRef,
      computerRef: input.computerRef,
      workspaceRef: input.workspaceRef,
      runtimeRef: input.runtimeRef,
      providerLeaseRef: input.providerLeaseRef,
      nonce: input.nonce,
    }))
      ref(value, field);
    digest(input.authorityDigest, "authorityDigest");
    integer(input.runtimeGeneration, "runtimeGeneration", 1);
    const ttl = time(input.expiresAt, "expiresAt") - time(input.issuedAt, "issuedAt");
    if (ttl <= 0 || ttl > maxTtlMs || !(await adapter.authorize(input)))
      throw new CloudComputerCommandError("scope_mismatch", "credential.authority");
    const value = Object.freeze({ ...input }) as CloudComputerReverseDialCredential;
    credentials.add(value);
    return value;
  },
});

export type CloudComputerRuntimeReservation = Readonly<{
  schema: typeof CLOUD_COMPUTER_RUNTIME_RESERVATION_SCHEMA;
  reservationRef: string;
  commandRef: string;
  requestDigest: CommandDigest;
  providerExecutionRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  providerLeaseRef: string;
  fence: number;
  reservedAt: string;
}>;
const runtimeReservations = new WeakSet<object>();
export function assertCloudComputerRuntimeReservation(
  value: unknown,
): asserts value is CloudComputerRuntimeReservation {
  if (typeof value !== "object" || value === null || !runtimeReservations.has(value)) {
    throw new CloudComputerCommandError("conflict", "runtimeReservation.authority");
  }
}
export type CloudComputerCommandEventKind =
  | "accepted"
  | "stdout"
  | "stderr"
  | "progress"
  | "tool"
  | "lifecycle"
  | "checkpoint"
  | "completed"
  | "failed"
  | "cancelled"
  | "timed_out"
  | "lost";
export type CloudComputerCommandEventInput = Readonly<{
  eventRef: string;
  commandRef: string;
  requestDigest: CommandDigest;
  providerExecutionRef: string;
  sessionRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  sequence: number;
  fence: number;
  kind: CloudComputerCommandEventKind;
  payload: Readonly<Record<string, unknown>>;
  observedAt: string;
}>;
const eventBytes = (input: CloudComputerCommandEventInput) => ({
  schema: CLOUD_COMPUTER_COMMAND_EVENT_SCHEMA,
  eventRef: input.eventRef,
  commandRef: input.commandRef,
  requestDigest: input.requestDigest,
  providerExecutionRef: input.providerExecutionRef,
  sessionRef: input.sessionRef,
  runtimeRef: input.runtimeRef,
  runtimeGeneration: input.runtimeGeneration,
  sequence: input.sequence,
  fence: input.fence,
  kind: input.kind,
  payload: input.payload,
  observedAt: input.observedAt,
});
export type CloudComputerCommandEvent = ReturnType<typeof eventBytes> &
  Readonly<{ eventDigest: CommandDigest }>;
export const createCloudComputerCommandEvent = (
  input: CloudComputerCommandEventInput,
): CloudComputerCommandEvent => {
  for (const [field, value] of Object.entries({
    eventRef: input.eventRef,
    commandRef: input.commandRef,
    providerExecutionRef: input.providerExecutionRef,
    sessionRef: input.sessionRef,
    runtimeRef: input.runtimeRef,
  }))
    ref(value, field);
  digest(input.requestDigest, "requestDigest");
  integer(input.runtimeGeneration, "runtimeGeneration", 1);
  integer(input.sequence, "sequence", 1);
  integer(input.fence, "fence", 1);
  time(input.observedAt, "observedAt");
  if (
    ![
      "accepted",
      "stdout",
      "stderr",
      "progress",
      "tool",
      "lifecycle",
      "checkpoint",
      "completed",
      "failed",
      "cancelled",
      "timed_out",
      "lost",
    ].includes(input.kind) ||
    input.payload === null ||
    typeof input.payload !== "object" ||
    Array.isArray(input.payload)
  )
    throw new CloudComputerCommandError("invalid", "eventPayload");
  const bytes = eventBytes(input);
  return Object.freeze({ ...bytes, eventDigest: hash(bytes) });
};
export function assertCloudComputerCommandEvent(
  value: CloudComputerCommandEvent,
): asserts value is CloudComputerCommandEvent {
  if (createCloudComputerCommandEvent(value).eventDigest !== value.eventDigest) {
    throw new CloudComputerCommandError("conflict", "eventDigest");
  }
}
export type CloudComputerCommandCursor = Readonly<{
  schema: typeof CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA;
  sessionRef: string;
  commandRef: string;
  requestDigest: CommandDigest;
  providerExecutionRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  nextSequence: number;
  retentionEpoch: number;
  retainedThrough: number;
}>;
export type CloudComputerCommandTerminalEvidence = Readonly<{
  schema: typeof CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA;
  terminalRef: string;
  commandRef: string;
  requestDigest: CommandDigest;
  providerExecutionRef: string;
  sessionRef: string;
  runtimeRef: string;
  runtimeGeneration: number;
  fence: number;
  sequence: number;
  outcome: "completed" | "failed" | "cancelled" | "timed_out" | "lost";
  exitCode: number | null;
  outputDigest: CommandDigest | null;
  reason: string | null;
  observedAt: string;
  eventDigest: CommandDigest;
  evidenceDigest: CommandDigest;
}>;
export type CloudComputerRuntimeAcknowledgement = Readonly<{
  commandRef: string;
  requestDigest: CommandDigest;
  reservationRef: string;
  providerExecutionRef: string;
  eventRef: string;
  eventDigest: CommandDigest;
  observedAt: string;
  acceptedSequence: number;
  fence: number;
  replayed: boolean;
}>;
const runtimeAcknowledgements = new WeakSet<object>();
export function assertCloudComputerRuntimeAcknowledgement(
  value: unknown,
): asserts value is CloudComputerRuntimeAcknowledgement {
  if (typeof value !== "object" || value === null || !runtimeAcknowledgements.has(value)) {
    throw new CloudComputerCommandError("conflict", "runtimeAcknowledgement.authority");
  }
}
type State = {
  command: CloudComputerCommand;
  fence: number;
  reservation: CloudComputerRuntimeReservation | null;
  acknowledgement: Readonly<{ acceptedSequence: number; fence: number }> | null;
  cancellation: Readonly<{
    ref: string;
    reason: string;
    requestedAt: string;
    fence: number;
  }> | null;
  terminal: CloudComputerCommandTerminalEvidence | null;
  events: Map<number, CloudComputerCommandEvent>;
  byRef: Map<string, CloudComputerCommandEvent>;
  lastSequence: number;
  retainedThrough: number;
  retentionEpoch: number;
};
const terminalKinds = new Set<CloudComputerCommandEventKind>([
  "completed",
  "failed",
  "cancelled",
  "timed_out",
  "lost",
]);

export class CloudComputerReverseDialSession {
  readonly #states = new Map<string, State>();
  readonly #idempotency = new Map<string, State>();
  readonly #nonces = new Set<string>();
  #epoch = 0;
  constructor(
    readonly scope: Readonly<{
      sessionRef: string;
      ownerRef: string;
      tenantRef: string;
      computerRef: string;
      workspaceRef: string;
      runtimeRef: string;
      runtimeGeneration: number;
      providerLeaseRef: string;
      authorityDigest: CommandDigest;
    }>,
  ) {
    for (const [field, value] of Object.entries(scope))
      if (typeof value === "string" && field !== "authorityDigest") ref(value, field);
    integer(scope.runtimeGeneration, "runtimeGeneration", 1);
    digest(scope.authorityDigest, "authorityDigest");
  }
  admit(command: CloudComputerCommand) {
    this.#command(command);
    const replay = this.#idempotency.get(command.idempotencyRef);
    if (replay) {
      if (replay.command.requestDigest !== command.requestDigest)
        throw new CloudComputerCommandError("conflict", "idempotencyRef");
      return { command: replay.command, replayed: true };
    }
    if (this.#states.has(command.commandRef))
      throw new CloudComputerCommandError("conflict", "commandRef");
    const state: State = {
      command,
      fence: 1,
      reservation: null,
      acknowledgement: null,
      cancellation: null,
      terminal: null,
      events: new Map(),
      byRef: new Map(),
      lastSequence: 0,
      retainedThrough: 0,
      retentionEpoch: 0,
    };
    this.#states.set(command.commandRef, state);
    this.#idempotency.set(command.idempotencyRef, state);
    return { command, replayed: false };
  }
  attach(
    input: Readonly<{
      connectionRef: string;
      credential: CloudComputerReverseDialCredential;
      now: string;
      cursors: ReadonlyArray<CloudComputerCommandCursor>;
    }>,
  ) {
    ref(input.connectionRef, "connectionRef");
    const now = time(input.now, "now");
    const c = input.credential;
    if (
      !credentials.has(c) ||
      this.#nonces.has(c.nonce) ||
      now < Date.parse(c.issuedAt) ||
      now >= Date.parse(c.expiresAt) ||
      c.sessionRef !== this.scope.sessionRef ||
      c.ownerRef !== this.scope.ownerRef ||
      c.tenantRef !== this.scope.tenantRef ||
      c.computerRef !== this.scope.computerRef ||
      c.workspaceRef !== this.scope.workspaceRef ||
      c.runtimeRef !== this.scope.runtimeRef ||
      c.runtimeGeneration !== this.scope.runtimeGeneration ||
      c.providerLeaseRef !== this.scope.providerLeaseRef ||
      c.authorityDigest !== this.scope.authorityDigest
    )
      throw new CloudComputerCommandError("scope_mismatch", "credential");
    const cursors = new Map(input.cursors.map((cursor) => [cursor.commandRef, cursor]));
    if (cursors.size !== input.cursors.length)
      throw new CloudComputerCommandError("conflict", "cursors");
    if ([...cursors.keys()].some((commandRef) => !this.#states.has(commandRef)))
      throw new CloudComputerCommandError("not_found", "cursor.commandRef");
    const executions = [...this.#states.values()]
      .filter((state) => !state.terminal)
      .map((state) => ({
        command: state.command,
        fence: state.fence,
        reservation: state.reservation,
        acknowledgement: state.acknowledgement,
        events: cursors.has(state.command.commandRef)
          ? this.#resume(state, cursors.get(state.command.commandRef)!)
          : [...state.events.values()],
        cursor: state.reservation ? this.#cursor(state) : null,
      }));
    const attachmentEpoch = this.#epoch + 1;
    this.#nonces.add(c.nonce);
    this.#epoch = attachmentEpoch;
    return { attachmentEpoch, executions };
  }
  reserve(
    input: Readonly<{
      attachmentEpoch: number;
      commandRef: string;
      requestDigest: CommandDigest;
      reservationRef: string;
      providerExecutionRef: string;
      reservedAt: string;
    }>,
  ) {
    this.#attachment(input.attachmentEpoch);
    const state = this.#state(input.commandRef, input.requestDigest);
    if (state.terminal) throw new CloudComputerCommandError("terminal", "commandRef");
    if (state.reservation) {
      if (
        state.reservation.reservationRef !== input.reservationRef ||
        state.reservation.providerExecutionRef !== input.providerExecutionRef
      )
        throw new CloudComputerCommandError("conflict", "reservation");
      return { reservation: state.reservation, replayed: true };
    }
    ref(input.reservationRef, "reservationRef");
    ref(input.providerExecutionRef, "providerExecutionRef");
    if (time(input.reservedAt, "reservedAt") > Date.parse(state.command.deadlineAt))
      throw new CloudComputerCommandError("deadline_expired", "reservedAt");
    state.reservation = Object.freeze({
      schema: CLOUD_COMPUTER_RUNTIME_RESERVATION_SCHEMA,
      reservationRef: input.reservationRef,
      commandRef: input.commandRef,
      requestDigest: input.requestDigest,
      providerExecutionRef: input.providerExecutionRef,
      runtimeRef: this.scope.runtimeRef,
      runtimeGeneration: this.scope.runtimeGeneration,
      providerLeaseRef: this.scope.providerLeaseRef,
      fence: state.fence,
      reservedAt: input.reservedAt,
    });
    runtimeReservations.add(state.reservation);
    return { reservation: state.reservation, replayed: false };
  }
  acknowledge(
    input: Readonly<{
      attachmentEpoch: number;
      commandRef: string;
      requestDigest: CommandDigest;
      reservationRef: string;
      providerExecutionRef: string;
      eventRef: string;
      sequence: number;
      observedAt: string;
    }>,
  ): CloudComputerRuntimeAcknowledgement {
    this.#attachment(input.attachmentEpoch);
    const state = this.#state(input.commandRef, input.requestDigest);
    const reservation = state.reservation;
    if (
      !reservation ||
      reservation.reservationRef !== input.reservationRef ||
      reservation.providerExecutionRef !== input.providerExecutionRef
    )
      throw new CloudComputerCommandError("conflict", "runtimeReservationBoundary");
    if (time(input.observedAt, "observedAt") > Date.parse(state.command.deadlineAt))
      throw new CloudComputerCommandError("deadline_expired", "observedAt");
    if (state.acknowledgement) {
      const event = state.events.get(state.acknowledgement.acceptedSequence);
      if (event?.eventRef !== input.eventRef || input.sequence !== event.sequence)
        throw new CloudComputerCommandError("conflict", "acknowledgement");
      const acknowledgement = {
        commandRef: input.commandRef,
        requestDigest: input.requestDigest,
        reservationRef: input.reservationRef,
        providerExecutionRef: input.providerExecutionRef,
        eventRef: event.eventRef,
        eventDigest: event.eventDigest,
        observedAt: event.observedAt,
        ...state.acknowledgement,
        replayed: true,
      };
      runtimeAcknowledgements.add(acknowledgement);
      return acknowledgement;
    }
    const event = createCloudComputerCommandEvent({
      eventRef: input.eventRef,
      commandRef: input.commandRef,
      requestDigest: input.requestDigest,
      providerExecutionRef: input.providerExecutionRef,
      sessionRef: this.scope.sessionRef,
      runtimeRef: this.scope.runtimeRef,
      runtimeGeneration: this.scope.runtimeGeneration,
      sequence: input.sequence,
      fence: state.fence,
      kind: "accepted",
      payload: {
        reservationRef: input.reservationRef,
        providerExecutionRef: input.providerExecutionRef,
      },
      observedAt: input.observedAt,
    });
    this.#append(state, event);
    state.acknowledgement = { acceptedSequence: event.sequence, fence: state.fence };
    const acknowledgement = {
      commandRef: input.commandRef,
      requestDigest: input.requestDigest,
      reservationRef: input.reservationRef,
      providerExecutionRef: input.providerExecutionRef,
      eventRef: event.eventRef,
      eventDigest: event.eventDigest,
      observedAt: event.observedAt,
      ...state.acknowledgement,
      replayed: false,
    };
    runtimeAcknowledgements.add(acknowledgement);
    return acknowledgement;
  }
  append(
    input: Readonly<{
      attachmentEpoch: number;
      event: CloudComputerCommandEvent;
      recoveryEvidence?: CloudComputerCommandRecoveryEvidence;
    }>,
  ) {
    this.#attachment(input.attachmentEpoch);
    const state = this.#state(input.event.commandRef, input.event.requestDigest);
    assertCloudComputerCommandEvent(input.event);
    const prior = state.byRef.get(input.event.eventRef);
    if (prior) {
      if (prior.eventDigest !== input.event.eventDigest)
        throw new CloudComputerCommandError("conflict", "eventRef");
      return { event: prior, replayed: true, terminal: state.terminal };
    }
    if (!state.reservation || !state.acknowledgement)
      throw new CloudComputerCommandError("conflict", "acknowledgementBoundary");
    this.#event(state, input.event);
    if (input.event.fence !== state.fence)
      throw new CloudComputerCommandError("stale_fence", "fence");
    if (state.terminal) throw new CloudComputerCommandError("terminal", "commandRef");
    if (Date.parse(input.event.observedAt) > Date.parse(state.command.deadlineAt))
      throw new CloudComputerCommandError("deadline_expired", "observedAt");
    if (input.event.kind === "accepted" || input.event.kind === "timed_out")
      throw new CloudComputerCommandError("invalid", "event.kind");
    if (state.cancellation && input.event.kind !== "cancelled")
      throw new CloudComputerCommandError("stale_fence", "cancellation");
    if (input.event.kind === "lost") this.#lossEvidence(input.event, input.recoveryEvidence);
    const terminal = terminalKinds.has(input.event.kind) ? this.#terminal(input.event) : null;
    this.#append(state, input.event);
    if (terminal) state.terminal = terminal;
    return { event: input.event, replayed: false, terminal };
  }
  requestCancellation(
    input: Readonly<{
      cancellationRef: string;
      commandRef: string;
      requestDigest: CommandDigest;
      expectedFence: number;
      reason: string;
      requestedAt: string;
    }>,
  ) {
    ref(input.cancellationRef, "cancellationRef");
    const state = this.#state(input.commandRef, input.requestDigest);
    if (state.terminal) throw new CloudComputerCommandError("terminal", "commandRef");
    if (state.cancellation) {
      if (
        state.cancellation.ref !== input.cancellationRef ||
        state.cancellation.reason !== input.reason ||
        state.cancellation.requestedAt !== input.requestedAt
      )
        throw new CloudComputerCommandError("conflict", "cancellationRef");
      return { commandRef: input.commandRef, fence: state.cancellation.fence, replayed: true };
    }
    if (input.expectedFence !== state.fence)
      throw new CloudComputerCommandError("stale_fence", "fence");
    if (time(input.requestedAt, "requestedAt") > Date.parse(state.command.deadlineAt))
      throw new CloudComputerCommandError("deadline_expired", "requestedAt");
    state.fence += 1;
    state.cancellation = {
      ref: input.cancellationRef,
      reason: input.reason,
      requestedAt: input.requestedAt,
      fence: state.fence,
    };
    return { commandRef: input.commandRef, fence: state.fence, replayed: false };
  }
  timeout(
    input: Readonly<{
      commandRef: string;
      requestDigest: CommandDigest;
      expectedFence: number;
      eventRef: string;
      observedAt: string;
      reason: string;
    }>,
  ): CloudComputerCommandTerminalEvidence {
    const state = this.#state(input.commandRef, input.requestDigest);
    if (state.terminal) {
      if (state.terminal.terminalRef !== input.eventRef || state.terminal.outcome !== "timed_out")
        throw new CloudComputerCommandError("conflict", "terminalRef");
      return state.terminal;
    }
    if (!state.reservation)
      throw new CloudComputerCommandError("not_found", "providerExecutionRef");
    if (input.expectedFence !== state.fence)
      throw new CloudComputerCommandError("stale_fence", "fence");
    if (time(input.observedAt, "observedAt") < Date.parse(state.command.deadlineAt))
      throw new CloudComputerCommandError("invalid", "timeoutBeforeDeadline");
    state.fence += 1;
    const event = createCloudComputerCommandEvent({
      eventRef: input.eventRef,
      commandRef: state.command.commandRef,
      requestDigest: state.command.requestDigest,
      providerExecutionRef: state.reservation.providerExecutionRef,
      sessionRef: this.scope.sessionRef,
      runtimeRef: this.scope.runtimeRef,
      runtimeGeneration: this.scope.runtimeGeneration,
      sequence: state.lastSequence + 1,
      fence: state.fence,
      kind: "timed_out",
      payload: { exitCode: null, outputDigest: null, reason: input.reason },
      observedAt: input.observedAt,
    });
    const terminal = this.#terminal(event);
    this.#append(state, event);
    state.terminal = terminal;
    return terminal;
  }
  cursor(commandRef: string) {
    const state = this.#states.get(commandRef);
    if (!state?.reservation)
      throw new CloudComputerCommandError("not_found", "providerExecutionRef");
    return this.#cursor(state);
  }
  compactThrough(commandRef: string, sequence: number) {
    const state = this.#states.get(commandRef);
    if (!state) throw new CloudComputerCommandError("not_found", "commandRef");
    integer(sequence, "sequence");
    if (sequence < state.retainedThrough || sequence > state.lastSequence)
      throw new CloudComputerCommandError("cursor_regressed", "sequence");
    for (const [number, event] of state.events)
      if (number <= sequence) {
        state.events.delete(number);
        state.byRef.delete(event.eventRef);
      }
    state.retainedThrough = sequence;
    state.retentionEpoch += 1;
  }
  #attachment(epoch: number) {
    if (epoch === 0 || epoch !== this.#epoch)
      throw new CloudComputerCommandError("stale_attachment", "attachmentEpoch");
  }
  #state(commandRef: string, requestDigest: CommandDigest) {
    const state = this.#states.get(commandRef);
    if (!state) throw new CloudComputerCommandError("not_found", "commandRef");
    if (state.command.requestDigest !== requestDigest)
      throw new CloudComputerCommandError("conflict", "requestDigest");
    return state;
  }
  #command(command: CloudComputerCommand) {
    assertCloudComputerCommand(command);
    if (
      command.ownerRef !== this.scope.ownerRef ||
      command.tenantRef !== this.scope.tenantRef ||
      command.computerRef !== this.scope.computerRef ||
      command.workspaceRef !== this.scope.workspaceRef ||
      command.runtimeRef !== this.scope.runtimeRef ||
      command.runtimeGeneration !== this.scope.runtimeGeneration ||
      command.providerLeaseRef !== this.scope.providerLeaseRef ||
      command.authorityDigest !== this.scope.authorityDigest
    )
      throw new CloudComputerCommandError("scope_mismatch", "command");
  }
  #event(state: State, event: CloudComputerCommandEvent) {
    if (
      !state.reservation ||
      event.providerExecutionRef !== state.reservation.providerExecutionRef ||
      event.sessionRef !== this.scope.sessionRef ||
      event.runtimeRef !== this.scope.runtimeRef ||
      event.runtimeGeneration !== this.scope.runtimeGeneration ||
      createCloudComputerCommandEvent(event).eventDigest !== event.eventDigest
    )
      throw new CloudComputerCommandError("scope_mismatch", "event");
  }
  #lossEvidence(
    event: CloudComputerCommandEvent,
    evidence: CloudComputerCommandRecoveryEvidence | undefined,
  ) {
    try {
      assertCloudComputerCommandRecoveryEvidence(evidence);
    } catch {
      throw new CloudComputerCommandError("conflict", "lossEvidence.authority");
    }
    if (
      (evidence.kind !== "runtime_lost" && evidence.kind !== "host_lost") ||
      evidence.computerRef !== this.scope.computerRef ||
      evidence.workspaceRef !== this.scope.workspaceRef ||
      evidence.runtimeRef !== this.scope.runtimeRef ||
      evidence.runtimeGeneration !== this.scope.runtimeGeneration ||
      evidence.providerLeaseRef !== this.scope.providerLeaseRef ||
      Date.parse(evidence.observedAt) > Date.parse(event.observedAt) ||
      event.payload.lossEvidenceRef !== evidence.evidenceRef ||
      event.payload.lossEvidenceDigest !== evidence.evidenceDigest
    ) {
      throw new CloudComputerCommandError("scope_mismatch", "lossEvidence");
    }
  }
  #append(state: State, event: CloudComputerCommandEvent) {
    if (event.sequence !== state.lastSequence + 1)
      throw new CloudComputerCommandError("cursor_regressed", "event.sequence");
    state.events.set(event.sequence, event);
    state.byRef.set(event.eventRef, event);
    state.lastSequence = event.sequence;
  }
  #cursor(state: State): CloudComputerCommandCursor {
    return Object.freeze({
      schema: CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA,
      sessionRef: this.scope.sessionRef,
      commandRef: state.command.commandRef,
      requestDigest: state.command.requestDigest,
      providerExecutionRef: state.reservation!.providerExecutionRef,
      runtimeRef: this.scope.runtimeRef,
      runtimeGeneration: this.scope.runtimeGeneration,
      nextSequence: state.lastSequence + 1,
      retentionEpoch: state.retentionEpoch,
      retainedThrough: state.retainedThrough,
    });
  }
  #resume(state: State, cursor: CloudComputerCommandCursor) {
    if (
      !state.reservation ||
      cursor.schema !== CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA ||
      cursor.sessionRef !== this.scope.sessionRef ||
      cursor.commandRef !== state.command.commandRef ||
      cursor.requestDigest !== state.command.requestDigest ||
      cursor.providerExecutionRef !== state.reservation.providerExecutionRef ||
      cursor.runtimeRef !== this.scope.runtimeRef ||
      cursor.runtimeGeneration !== this.scope.runtimeGeneration
    )
      throw new CloudComputerCommandError("scope_mismatch", "cursor");
    integer(cursor.nextSequence, "cursor.nextSequence", 1);
    if (
      cursor.retentionEpoch !== state.retentionEpoch ||
      cursor.retainedThrough !== state.retainedThrough ||
      cursor.nextSequence <= state.retainedThrough
    )
      throw new CloudComputerCommandError("cursor_expired", "cursor");
    if (cursor.nextSequence > state.lastSequence + 1)
      throw new CloudComputerCommandError("cursor_regressed", "cursor.nextSequence");
    return [...state.events.values()].filter((event) => event.sequence >= cursor.nextSequence);
  }
  #terminal(event: CloudComputerCommandEvent): CloudComputerCommandTerminalEvidence {
    const exitCode = event.payload.exitCode;
    const outputDigest = event.payload.outputDigest;
    const reason = event.payload.reason;
    if (
      !(exitCode === null || (Number.isSafeInteger(exitCode) && Number(exitCode) >= 0)) ||
      !(outputDigest === null || (typeof outputDigest === "string" && DIGEST.test(outputDigest))) ||
      !(
        reason === null ||
        (typeof reason === "string" && /^[a-z][a-z0-9_]{0,63}$/u.test(reason))
      ) ||
      (event.kind === "completed" && exitCode !== 0) ||
      (event.kind !== "completed" && reason === null)
    )
      throw new CloudComputerCommandError("invalid", "terminal.payload");
    const bytes = {
      schema: CLOUD_COMPUTER_COMMAND_TERMINAL_SCHEMA,
      terminalRef: event.eventRef,
      commandRef: event.commandRef,
      requestDigest: event.requestDigest,
      providerExecutionRef: event.providerExecutionRef,
      sessionRef: event.sessionRef,
      runtimeRef: event.runtimeRef,
      runtimeGeneration: event.runtimeGeneration,
      fence: event.fence,
      sequence: event.sequence,
      outcome: event.kind as CloudComputerCommandTerminalEvidence["outcome"],
      exitCode: exitCode as number | null,
      outputDigest: outputDigest as CommandDigest | null,
      reason: reason as string | null,
      observedAt: event.observedAt,
      eventDigest: event.eventDigest,
    };
    return Object.freeze({ ...bytes, evidenceDigest: hash(bytes) });
  }
}
