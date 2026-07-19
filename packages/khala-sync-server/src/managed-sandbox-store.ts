import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  advanceSandboxModelGeneration,
  applySandboxModelEvent,
  BoxProjectionCursorSchema,
  initialSandboxModelState,
  ManagedSandboxCommandSchema,
  ManagedSandboxEventSchema,
  ManagedSandboxReceiptSchema,
  ManagedSandboxResourceSchema,
  ManagedSandboxRuntimeEventInputSchema,
  ManagedSandboxTurnReceiptSchema,
  ManagedSandboxTurnSchema,
  SandboxRef,
  type BoxProjectionCursor,
  type ManagedSandboxCommand,
  type ManagedSandboxEvent,
  type ManagedSandboxReceipt,
  type ManagedSandboxResource,
  type ManagedSandboxRuntimeEventInput,
  type ManagedSandboxTurn,
  type ManagedSandboxTurnReceipt,
  type SandboxModelState,
} from "@openagentsinc/managed-sandbox-contract";
import { Schema as S } from "effect";

import type { SyncSql, SyncTransactionSql } from "./sql.js";

const decodeCommand = S.decodeUnknownSync(ManagedSandboxCommandSchema);
const decodeEvent = S.decodeUnknownSync(ManagedSandboxEventSchema);
const decodeReceipt = S.decodeUnknownSync(ManagedSandboxReceiptSchema);
const decodeResource = S.decodeUnknownSync(ManagedSandboxResourceSchema);
const decodeRuntimeEventInput = S.decodeUnknownSync(ManagedSandboxRuntimeEventInputSchema);
const decodeTurn = S.decodeUnknownSync(ManagedSandboxTurnSchema);
const decodeTurnReceipt = S.decodeUnknownSync(ManagedSandboxTurnReceiptSchema);
const decodeProjectionCursor = S.decodeUnknownSync(BoxProjectionCursorSchema);
const decodeRef = S.decodeUnknownSync(SandboxRef);

const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

type StoreErrorCode =
  | "invalid"
  | "not_found"
  | "permission_denied"
  | "idempotency_conflict"
  | "command_conflict"
  | "stale_version"
  | "stale_generation"
  | "invalid_transition"
  | "event_conflict"
  | "cursor_conflict"
  | "unsafe_value"
  | "corrupt_store";

export class ManagedSandboxStoreError extends Error {
  readonly _tag = "ManagedSandboxStoreError";
  override readonly name = "ManagedSandboxStoreError";

  constructor(
    readonly code: StoreErrorCode,
    message: string,
  ) {
    super(message);
  }
}

type ResourceRow = Readonly<{
  sandbox_ref: string;
  owner_user_id: string;
  tenant_ref: string;
  resource_generation: string | number;
  version: string | number;
  last_event_sequence: string | number;
  active_command_ref: string | null;
  resource_json: unknown;
}>;

type CommandRow = Readonly<{
  command_ref: string;
  sandbox_ref: string;
  owner_user_id: string;
  tenant_ref: string;
  idempotency_ref: string;
  command_fingerprint: string;
  settlement_fingerprint: string | null;
  command_json: unknown;
  resource_generation: string | number;
  claimed_version: string | number;
  status: "pending" | "settled" | "recovery_required" | "refused";
  receipt_ref: string | null;
}>;

type EventRow = Readonly<{ event_json: unknown }>;
type ReceiptRow = Readonly<{ receipt_json: unknown }>;
type TurnRow = Readonly<{
  turn_sequence: string | number;
  turn_ref: string;
  status: string;
}>;
type RuntimeTurnRow = TurnRow &
  Readonly<{
    resource_generation: string | number;
    last_event_sequence: string | number;
    command_ref: string;
    interrupt_command_ref: string | null;
    turn_json: unknown;
    turn_receipt_json: unknown | null;
  }>;
type ProjectionRow = Readonly<{
  projection_version: string | number;
  native_event_sequence: string | number;
  cursor_json: unknown;
}>;

export type ManagedSandboxCommandReservation = Readonly<{
  disposition: "reserved" | "replayed" | "settled";
  status: CommandRow["status"];
  command: ManagedSandboxCommand;
  resource: ManagedSandboxResource;
  receipt?: ManagedSandboxReceipt | undefined;
  turnSequence?: number | undefined;
}>;

export type ManagedSandboxEventPage = Readonly<{
  sandboxRef: string;
  afterSequence: number;
  nextSequence: number;
  terminalSequence: number;
  events: ReadonlyArray<ManagedSandboxEvent>;
}>;

export type ManagedSandboxPendingCommand = Readonly<{
  command: ManagedSandboxCommand;
  resource: ManagedSandboxResource;
  claimedVersion: number;
  resourceGeneration: number;
}>;

export type ManagedSandboxTurnOrder = Readonly<{
  turnSequence: number;
  turnRef: string;
  status: "pending" | "running" | "interrupting" | "settled" | "failed" | "interrupted";
}>;

export type ManagedSandboxRuntimeEventPage = Readonly<{
  turn: ManagedSandboxTurn;
  events: ReadonlyArray<ManagedSandboxEvent>;
  afterTurnSequence: number;
  nextTurnSequence: number;
  terminalTurnSequence: number;
}>;

export type RecordManagedSandboxRuntimeEventsInput = Readonly<{
  ownerRef: string;
  tenantRef: string;
  sandboxRef: string;
  turnRef: string;
  expectedResourceGeneration: number;
  events: ReadonlyArray<unknown>;
  evidenceRefs?: ReadonlyArray<string> | undefined;
}>;

export type RecordManagedSandboxRuntimeEventsResult = Readonly<{
  turn: ManagedSandboxTurn;
  receipt?: ManagedSandboxTurnReceipt | undefined;
  events: ReadonlyArray<ManagedSandboxEvent>;
}>;

export type ManagedSandboxProjectionState = Readonly<{
  projectionVersion: number;
  cursor: BoxProjectionCursor;
}>;

export type ReserveManagedSandboxCommandInput = Readonly<{
  command: unknown;
  initialResource?: unknown;
}>;

export type SettleManagedSandboxCommandInput = Readonly<{
  ownerRef: string;
  tenantRef: string;
  sandboxRef: string;
  commandRef: string;
  expectedResourceGeneration: number;
  events: ReadonlyArray<unknown>;
  outcome: "succeeded" | "failed" | "refused";
  artifactRefs?: ReadonlyArray<string> | undefined;
  errorCode?: string | undefined;
  observedAt: string;
}>;

export type AdvanceManagedSandboxProjectionInput = Readonly<{
  ownerRef: string;
  tenantRef: string;
  sandboxRef: string;
  expectedProjectionVersion: number;
  cursor: unknown;
  observedAt: string;
}>;

const parseJson = (value: unknown): unknown =>
  typeof value === "string" ? JSON.parse(value) : value;

const integer = (value: string | number, field: string): number => {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) {
    throw new ManagedSandboxStoreError("corrupt_store", `${field} is not a safe integer`);
  }
  return parsed;
};

const same = (left: unknown, right: unknown): boolean =>
  canonicalJson(left) === canonicalJson(right);

const fingerprint = (value: unknown): string =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`;

const deterministicRef = (kind: "event" | "receipt", ...parts: ReadonlyArray<string>): string =>
  `${kind}.sbx.${createHash("sha256").update(parts.join("\u0000")).digest("hex").slice(0, 40)}`;

const publicSafe = <A>(value: A): A => {
  if (FORBIDDEN_PRIVATE_MATERIAL.test(canonicalJson(value))) {
    throw new ManagedSandboxStoreError(
      "unsafe_value",
      "managed sandbox durable value contains forbidden private material",
    );
  }
  return value;
};

const validateResourceBounds = (resource: ManagedSandboxResource): ManagedSandboxResource => {
  if (resource.facts.leaseState !== resource.lease.state) {
    throw new ManagedSandboxStoreError(
      "invalid",
      "resource lease fact does not match the exact lease",
    );
  }
  if (resource.budget.maxLifetimeSeconds > resource.lease.ttlSeconds) {
    throw new ManagedSandboxStoreError(
      "invalid",
      "resource budget lifetime exceeds the exact lease TTL",
    );
  }
  const capabilityRefs = resource.capabilities.map((item) => item.capabilityRef);
  if (capabilityRefs.length !== new Set(capabilityRefs).size) {
    throw new ManagedSandboxStoreError("invalid", "capability refs must be unique");
  }
  const leaseExpiry = Date.parse(resource.lease.expiresAt);
  if (resource.capabilities.some((item) => Date.parse(item.expiresAt) > leaseExpiry)) {
    throw new ManagedSandboxStoreError(
      "invalid",
      "capability expiry cannot exceed the resource lease",
    );
  }
  return resource;
};

const resourceFromRow = (row: ResourceRow): ManagedSandboxResource => {
  const resource = validateResourceBounds(publicSafe(decodeResource(parseJson(row.resource_json))));
  if (
    resource.sandboxRef !== row.sandbox_ref ||
    resource.ownerRef !== row.owner_user_id ||
    resource.tenantRef !== row.tenant_ref ||
    resource.resourceGeneration !== integer(row.resource_generation, "resource generation") ||
    resource.version !== integer(row.version, "resource version") ||
    resource.lastEventSequence !== integer(row.last_event_sequence, "event sequence")
  ) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "managed sandbox relational columns disagree with the canonical resource",
    );
  }
  return resource;
};

const commandFromRow = (row: CommandRow): ManagedSandboxCommand =>
  publicSafe(decodeCommand(parseJson(row.command_json)));

const receiptFromRow = (row: ReceiptRow): ManagedSandboxReceipt =>
  publicSafe(decodeReceipt(parseJson(row.receipt_json)));

const eventFromRow = (row: EventRow): ManagedSandboxEvent =>
  publicSafe(decodeEvent(parseJson(row.event_json)));

const turnFromRow = (row: RuntimeTurnRow): ManagedSandboxTurn => {
  const turn = publicSafe(decodeTurn(parseJson(row.turn_json)));
  if (
    turn.turnRef !== row.turn_ref ||
    turn.turnSequence !== integer(row.turn_sequence, "turn sequence") ||
    turn.resourceGeneration !== integer(row.resource_generation, "turn generation") ||
    turn.lastEventSequence !== integer(row.last_event_sequence, "turn event sequence") ||
    turn.commandRef !== row.command_ref ||
    turn.status !== row.status
  ) {
    throw new ManagedSandboxStoreError(
      "corrupt_store",
      "managed sandbox turn columns disagree with canonical turn state",
    );
  }
  return turn;
};

const turnReceiptFromRow = (row: RuntimeTurnRow): ManagedSandboxTurnReceipt | undefined =>
  row.turn_receipt_json === null
    ? undefined
    : publicSafe(decodeTurnReceipt(parseJson(row.turn_receipt_json)));

const runtimeEventForTurn = (
  event: ManagedSandboxEvent,
  turnRef: string,
): event is Extract<ManagedSandboxEvent, { readonly turnRef: string }> =>
  "turnRef" in event && event.turnRef === turnRef && "turnEventSequence" in event;

const runtimeInputFromEvent = (event: ManagedSandboxEvent): ManagedSandboxRuntimeEventInput => {
  if (!("turnRef" in event) || !("turnEventSequence" in event)) {
    throw new ManagedSandboxStoreError("corrupt_store", "event is not bound to a runtime turn");
  }
  const base = {
    _tag: event._tag,
    turnRef: event.turnRef,
    resourceGeneration: event.resourceGeneration,
    turnEventSequence: event.turnEventSequence,
    observedAt: event.observedAt,
  };
  switch (event._tag) {
    case "RuntimeStarted":
      return decodeRuntimeEventInput(base);
    case "RuntimeTextDelta":
      return decodeRuntimeEventInput({ ...base, content: event.content });
    case "RuntimeToolStarted":
      return decodeRuntimeEventInput({
        ...base,
        toolCallRef: event.toolCallRef,
        toolName: event.toolName,
      });
    case "RuntimeToolCompleted":
      return decodeRuntimeEventInput({
        ...base,
        toolCallRef: event.toolCallRef,
        toolName: event.toolName,
        outcome: event.outcome,
        evidenceRefs: event.evidenceRefs,
      });
    case "RuntimeUsageRecorded":
      return decodeRuntimeEventInput({ ...base, usage: event.usage });
    case "RuntimeInterruptRequested":
    case "RuntimeInterrupted":
      return decodeRuntimeEventInput({ ...base, reasonRef: event.reasonRef });
    case "RuntimeSettled":
      return decodeRuntimeEventInput({
        ...base,
        finishReason: event.finishReason,
        ...(event.usage === undefined ? {} : { usage: event.usage }),
      });
    case "RuntimeFailed":
      return decodeRuntimeEventInput({
        ...base,
        errorRef: event.errorRef,
        retryable: event.retryable,
      });
    default:
      throw new ManagedSandboxStoreError("corrupt_store", "event is not a runtime event");
  }
};

const materializeRuntimeEvent = (
  input: ManagedSandboxRuntimeEventInput,
  sandboxRef: string,
  globalSequence: number,
): ManagedSandboxEvent =>
  publicSafe(
    decodeEvent({
      ...input,
      schema: "openagents.managed_sandbox_event.v1",
      eventRef: deterministicRef("event", input.turnRef, String(input.turnEventSequence)),
      sandboxRef,
      sequence: globalSequence,
    }),
  );

const advanceTurn = (
  current: ManagedSandboxTurn,
  events: ReadonlyArray<ManagedSandboxEvent>,
): ManagedSandboxTurn => {
  let turn = current;
  for (const event of events) {
    if (!runtimeEventForTurn(event, turn.turnRef)) continue;
    if (event.resourceGeneration !== turn.resourceGeneration) {
      throw new ManagedSandboxStoreError(
        "stale_generation",
        "runtime event generation does not match the exact turn",
      );
    }
    if (event.turnEventSequence !== turn.lastEventSequence + 1) {
      throw new ManagedSandboxStoreError("event_conflict", "runtime event sequence is not dense");
    }
    const active = ["running", "interrupting"] as const;
    switch (event._tag) {
      case "RuntimeStarted":
        if (turn.status !== "pending") {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime start requires a pending turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          status: "running",
          startedAt: event.observedAt,
          lastEventSequence: event.turnEventSequence,
        });
        break;
      case "RuntimeTextDelta":
      case "RuntimeToolStarted":
      case "RuntimeToolCompleted":
        if (!active.includes(turn.status as (typeof active)[number])) {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            `${event._tag} requires an active turn`,
          );
        }
        turn = decodeTurn({ ...turn, lastEventSequence: event.turnEventSequence });
        break;
      case "RuntimeUsageRecorded":
        if (!active.includes(turn.status as (typeof active)[number])) {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime usage requires an active turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          usage: event.usage,
          lastEventSequence: event.turnEventSequence,
        });
        break;
      case "RuntimeInterruptRequested":
        if (turn.status !== "running") {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime interrupt requires a running turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          status: "interrupting",
          lastEventSequence: event.turnEventSequence,
        });
        break;
      case "RuntimeSettled":
        if (!active.includes(turn.status as (typeof active)[number])) {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime settlement requires an active turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          status: "settled",
          settledAt: event.observedAt,
          terminalReason: event.finishReason,
          ...(event.usage === undefined ? {} : { usage: event.usage }),
          lastEventSequence: event.turnEventSequence,
        });
        break;
      case "RuntimeFailed":
        if (!active.includes(turn.status as (typeof active)[number])) {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime failure requires an active turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          status: "failed",
          settledAt: event.observedAt,
          terminalReason: "provider_failure",
          lastEventSequence: event.turnEventSequence,
        });
        break;
      case "RuntimeInterrupted":
        if (!active.includes(turn.status as (typeof active)[number])) {
          throw new ManagedSandboxStoreError(
            "invalid_transition",
            "runtime interruption requires an active turn",
          );
        }
        turn = decodeTurn({
          ...turn,
          status: "interrupted",
          settledAt: event.observedAt,
          terminalReason: "explicit_stop",
          lastEventSequence: event.turnEventSequence,
        });
        break;
      default:
        break;
    }
  }
  return turn;
};

const makeTurnReceipt = (
  turn: ManagedSandboxTurn,
  terminalEventSequence: number,
  evidenceRefs: ReadonlyArray<string>,
): ManagedSandboxTurnReceipt | undefined => {
  if (
    turn.settledAt === undefined ||
    turn.terminalReason === undefined ||
    !["settled", "failed", "interrupted"].includes(turn.status)
  ) {
    return undefined;
  }
  return publicSafe(
    decodeTurnReceipt({
      schema: "openagents.managed_sandbox_turn_receipt.v1",
      receiptRef: deterministicRef("receipt", turn.turnRef, String(terminalEventSequence)),
      turnRef: turn.turnRef,
      sandboxRef: turn.sandboxRef,
      ownerRef: turn.ownerRef,
      tenantRef: turn.tenantRef,
      workUnitRef: turn.workUnitRef,
      resourceGeneration: turn.resourceGeneration,
      turnSequence: turn.turnSequence,
      terminalEventSequence,
      runtime: turn.runtime,
      outcome: turn.status,
      terminalReason: turn.terminalReason,
      ...(turn.usage === undefined ? {} : { usage: turn.usage }),
      evidenceRefs: evidenceRefs.map((value) => decodeRef(value)),
      observedAt: turn.settledAt,
    }),
  );
};

const modelFromResource = (resource: ManagedSandboxResource): SandboxModelState => ({
  lifecycle: resource.facts.lifecycle,
  resourceGeneration: resource.resourceGeneration,
  lastEventSequence: resource.lastEventSequence,
  acceptingWork: resource.facts.acceptingWork,
  guestState: resource.facts.guestState,
  filesystemState: resource.facts.filesystemState,
  ingressState: resource.facts.ingressState,
  runtimeState: resource.facts.runtimeState,
  cleanupComplete: resource.facts.cleanupComplete,
});

const withModel = (
  resource: ManagedSandboxResource,
  model: SandboxModelState,
  version: number,
  updatedAt: string,
): ManagedSandboxResource =>
  decodeResource({
    ...resource,
    resourceGeneration: model.resourceGeneration,
    version,
    lastEventSequence: model.lastEventSequence,
    facts: {
      ...resource.facts,
      lifecycle: model.lifecycle,
      guestState: model.guestState,
      filesystemState: model.filesystemState,
      ingressState: model.ingressState,
      runtimeState: model.runtimeState,
      acceptingWork: model.acceptingWork,
      cleanupComplete: model.cleanupComplete,
    },
    updatedAt,
  });

const validateNewResource = (
  command: Extract<ManagedSandboxCommand, { readonly _tag: "Create" }>,
  unknownResource: unknown,
): ManagedSandboxResource => {
  const resource = validateResourceBounds(publicSafe(decodeResource(unknownResource)));
  const initial = initialSandboxModelState(resource.resourceGeneration);
  if (
    resource.resourceGeneration < 1 ||
    resource.version !== 0 ||
    resource.lastEventSequence !== 0 ||
    resource.ownerRef !== command.ownerRef ||
    resource.tenantRef !== command.tenantRef ||
    resource.workUnitRef !== command.workUnitRef ||
    resource.attachmentRef !== command.attachmentRef ||
    !same(resource.target, command.target) ||
    resource.imageDigest !== command.imageDigest ||
    resource.profileRef !== command.profileRef ||
    !same(resource.lease, command.lease) ||
    !same(resource.budget, command.budget) ||
    !same(resource.capabilities, command.requestedCapabilities) ||
    !same(modelFromResource(resource), initial)
  ) {
    throw new ManagedSandboxStoreError(
      "invalid",
      "create command and initial managed sandbox resource do not bind exactly",
    );
  }
  return resource;
};

const selectResource = async (
  sql: SyncSql | SyncTransactionSql,
  sandboxRef: string,
  lock = false,
): Promise<ResourceRow | undefined> => {
  const rows: ReadonlyArray<ResourceRow> = lock
    ? await sql`
        SELECT sandbox_ref, owner_user_id, tenant_ref, resource_generation,
               version, last_event_sequence, active_command_ref, resource_json
        FROM khala_sync_managed_sandboxes
        WHERE sandbox_ref = ${sandboxRef}
        FOR UPDATE
      `
    : await sql`
        SELECT sandbox_ref, owner_user_id, tenant_ref, resource_generation,
               version, last_event_sequence, active_command_ref, resource_json
        FROM khala_sync_managed_sandboxes
        WHERE sandbox_ref = ${sandboxRef}
      `;
  return rows[0];
};

const selectCommand = async (
  sql: SyncSql | SyncTransactionSql,
  input: Readonly<{
    commandRef: string;
    ownerRef: string;
    tenantRef: string;
    idempotencyRef: string;
  }>,
  lock = false,
): Promise<CommandRow | undefined> => {
  const rows: ReadonlyArray<CommandRow> = lock
    ? await sql`
        SELECT command_ref, sandbox_ref, owner_user_id, tenant_ref,
               idempotency_ref, command_fingerprint, settlement_fingerprint, command_json,
               resource_generation, claimed_version, status, receipt_ref
        FROM khala_sync_managed_sandbox_commands
        WHERE command_ref = ${input.commandRef}
           OR (owner_user_id = ${input.ownerRef}
               AND tenant_ref = ${input.tenantRef}
               AND idempotency_ref = ${input.idempotencyRef})
        LIMIT 1
        FOR UPDATE
      `
    : await sql`
        SELECT command_ref, sandbox_ref, owner_user_id, tenant_ref,
               idempotency_ref, command_fingerprint, settlement_fingerprint, command_json,
               resource_generation, claimed_version, status, receipt_ref
        FROM khala_sync_managed_sandbox_commands
        WHERE command_ref = ${input.commandRef}
           OR (owner_user_id = ${input.ownerRef}
               AND tenant_ref = ${input.tenantRef}
               AND idempotency_ref = ${input.idempotencyRef})
        LIMIT 1
      `;
  return rows[0];
};

const receiptForCommand = async (
  sql: SyncSql | SyncTransactionSql,
  commandRef: string,
): Promise<ManagedSandboxReceipt | undefined> => {
  const rows: ReadonlyArray<ReceiptRow> = await sql`
    SELECT receipt_json
    FROM khala_sync_managed_sandbox_receipts
    WHERE command_ref = ${commandRef}
  `;
  return rows[0] === undefined ? undefined : receiptFromRow(rows[0]);
};

const turnForCommand = async (
  sql: SyncSql | SyncTransactionSql,
  commandRef: string,
): Promise<number | undefined> => {
  const rows: ReadonlyArray<TurnRow> = await sql`
    SELECT turn_sequence, turn_ref, status
    FROM khala_sync_managed_sandbox_turns
    WHERE command_ref = ${commandRef}
  `;
  return rows[0] === undefined ? undefined : integer(rows[0].turn_sequence, "turn sequence");
};

const selectRuntimeTurn = async (
  sql: SyncSql | SyncTransactionSql,
  sandboxRef: string,
  turnRef: string,
  lock = false,
): Promise<RuntimeTurnRow | undefined> => {
  const rows: ReadonlyArray<RuntimeTurnRow> = lock
    ? await sql`
        SELECT turn_sequence, turn_ref, status, resource_generation,
               last_event_sequence, command_ref, interrupt_command_ref,
               turn_json, turn_receipt_json
        FROM khala_sync_managed_sandbox_turns
        WHERE sandbox_ref = ${sandboxRef} AND turn_ref = ${turnRef}
        FOR UPDATE
      `
    : await sql`
        SELECT turn_sequence, turn_ref, status, resource_generation,
               last_event_sequence, command_ref, interrupt_command_ref,
               turn_json, turn_receipt_json
        FROM khala_sync_managed_sandbox_turns
        WHERE sandbox_ref = ${sandboxRef} AND turn_ref = ${turnRef}
      `;
  return rows[0];
};

const assertScope = (row: ResourceRow, ownerRef: string, tenantRef: string): void => {
  if (row.owner_user_id !== ownerRef || row.tenant_ref !== tenantRef) {
    throw new ManagedSandboxStoreError(
      "permission_denied",
      "managed sandbox does not belong to the exact owner and tenant scope",
    );
  }
};

const assertExpectedVersion = (
  command: ManagedSandboxCommand,
  resource: ManagedSandboxResource,
): void => {
  if ("expectedVersion" in command && command.expectedVersion !== resource.version) {
    throw new ManagedSandboxStoreError(
      "stale_version",
      `expected resource version ${resource.version}, received ${command.expectedVersion}`,
    );
  }
};

const intentKind = (
  command: ManagedSandboxCommand,
): "ProvisionRequested" | "StopRequested" | "ResumeRequested" | "DeleteRequested" | null => {
  switch (command._tag) {
    case "Create":
      return "ProvisionRequested";
    case "Stop":
      return "StopRequested";
    case "Resume":
      return "ResumeRequested";
    case "Delete":
      return "DeleteRequested";
    default:
      return null;
  }
};

const makeIntentEvent = (
  command: ManagedSandboxCommand,
  resource: ManagedSandboxResource,
): ManagedSandboxEvent | null => {
  const kind = intentKind(command);
  if (kind === null) return null;
  return decodeEvent({
    _tag: kind,
    schema: "openagents.managed_sandbox_event.v1",
    eventRef: deterministicRef(
      "event",
      command.commandRef,
      kind,
      String(resource.lastEventSequence + 1),
    ),
    sandboxRef: resource.sandboxRef,
    resourceGeneration: resource.resourceGeneration,
    sequence: resource.lastEventSequence + 1,
    observedAt: command.requestedAt,
  });
};

const applyEvents = (
  resource: ManagedSandboxResource,
  command: ManagedSandboxCommand,
  events: ReadonlyArray<ManagedSandboxEvent>,
): SandboxModelState => {
  let state = modelFromResource(resource);
  const first = events[0];
  if (
    command._tag === "Resume" &&
    first !== undefined &&
    first.resourceGeneration === state.resourceGeneration + 1
  ) {
    state = advanceSandboxModelGeneration(state, first.resourceGeneration);
  }
  for (const event of events) {
    if (event.sandboxRef !== resource.sandboxRef) {
      throw new ManagedSandboxStoreError("event_conflict", "event sandbox scope does not match");
    }
    try {
      state = applySandboxModelEvent(state, {
        kind: event._tag,
        resourceGeneration: event.resourceGeneration,
        sequence: event.sequence,
      });
    } catch (error) {
      throw new ManagedSandboxStoreError(
        error instanceof Error && /generation/u.test(error.message)
          ? "stale_generation"
          : "invalid_transition",
        error instanceof Error ? error.message : "managed sandbox transition refused",
      );
    }
  }
  return state;
};

const updateResource = async (
  tx: SyncTransactionSql,
  resource: ManagedSandboxResource,
  activeCommandRef: string | null,
): Promise<void> => {
  await tx`
    UPDATE khala_sync_managed_sandboxes
    SET attachment_ref = ${resource.attachmentRef},
        attachment_generation = ${resource.attachmentGeneration},
        resource_generation = ${resource.resourceGeneration},
        version = ${resource.version},
        last_event_sequence = ${resource.lastEventSequence},
        target_ref = ${resource.target.targetRef},
        lifecycle = ${resource.facts.lifecycle},
        lease_state = ${resource.facts.leaseState},
        lease_expires_at = ${resource.lease.expiresAt},
        guest_state = ${resource.facts.guestState},
        filesystem_state = ${resource.facts.filesystemState},
        ingress_state = ${resource.facts.ingressState},
        runtime_state = ${resource.facts.runtimeState},
        accepting_work = ${resource.facts.acceptingWork},
        cleanup_complete = ${resource.facts.cleanupComplete},
        active_command_ref = ${activeCommandRef},
        resource_json = ${resource}::jsonb,
        updated_at = ${resource.updatedAt}
    WHERE sandbox_ref = ${resource.sandboxRef}
  `;
};

const insertReceipt = async (
  tx: SyncTransactionSql,
  receipt: ManagedSandboxReceipt,
): Promise<void> => {
  await tx`
    INSERT INTO khala_sync_managed_sandbox_receipts
      (receipt_ref, command_ref, sandbox_ref, owner_user_id, tenant_ref,
       resource_generation, version, outcome, receipt_json, observed_at)
    VALUES
      (${receipt.receiptRef}, ${receipt.commandRef}, ${receipt.sandboxRef},
       ${receipt.ownerRef}, ${receipt.tenantRef}, ${receipt.resourceGeneration},
       ${receipt.version}, ${receipt.outcome}, ${receipt}::jsonb,
       ${receipt.observedAt})
  `;
};

const makeReceipt = (
  command: ManagedSandboxCommand,
  resource: ManagedSandboxResource,
  outcome: ManagedSandboxReceipt["outcome"],
  eventRefs: ReadonlyArray<string>,
  artifactRefs: ReadonlyArray<string>,
  errorCode: string | undefined,
  observedAt: string,
): ManagedSandboxReceipt =>
  publicSafe(
    decodeReceipt({
      schema: "openagents.managed_sandbox_receipt.v1",
      receiptRef: deterministicRef("receipt", command.commandRef),
      commandRef: command.commandRef,
      sandboxRef: resource.sandboxRef,
      ownerRef: resource.ownerRef,
      tenantRef: resource.tenantRef,
      resourceGeneration: resource.resourceGeneration,
      version: resource.version,
      outcome,
      lifecycle: resource.facts.lifecycle,
      eventRefs,
      artifactRefs: artifactRefs.map((value) => decodeRef(value)),
      ...(errorCode === undefined ? {} : { errorCode: decodeRef(errorCode) }),
      observedAt,
    }),
  );

const reservationReplay = async (
  tx: SyncTransactionSql,
  row: CommandRow,
  expectedFingerprint: string,
  expectedSandboxRef: string,
): Promise<ManagedSandboxCommandReservation> => {
  if (row.command_fingerprint !== expectedFingerprint || row.sandbox_ref !== expectedSandboxRef) {
    throw new ManagedSandboxStoreError(
      "idempotency_conflict",
      "command ref or idempotency ref is bound to different request bytes",
    );
  }
  const resourceRow = await selectResource(tx, row.sandbox_ref, true);
  if (resourceRow === undefined) {
    throw new ManagedSandboxStoreError("corrupt_store", "replayed command lost its resource");
  }
  return {
    disposition: "replayed",
    status: row.status,
    command: commandFromRow(row),
    resource: resourceFromRow(resourceRow),
    receipt: await receiptForCommand(tx, row.command_ref),
    turnSequence: await turnForCommand(tx, row.command_ref),
  };
};

const settledCommand = async (
  tx: SyncTransactionSql,
  command: ManagedSandboxCommand,
  resource: ManagedSandboxResource,
  status: "settled" | "refused",
  outcome: "succeeded" | "refused",
): Promise<ManagedSandboxCommandReservation> => {
  const receipt = makeReceipt(
    command,
    resource,
    outcome,
    [],
    [],
    outcome === "refused" ? "command.refused" : undefined,
    command.requestedAt,
  );
  await insertReceipt(tx, receipt);
  await tx`
    UPDATE khala_sync_managed_sandbox_commands
    SET status = ${status}, receipt_ref = ${receipt.receiptRef},
        updated_at = ${command.requestedAt}
    WHERE command_ref = ${command.commandRef}
  `;
  return { disposition: "settled", status, command, resource, receipt };
};

export class PostgresManagedSandboxStore {
  constructor(private readonly sql: SyncSql) {}

  async reservation(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      commandRef: string;
    }>,
  ): Promise<ManagedSandboxCommandReservation | undefined> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const commandRef = decodeRef(input.commandRef);
    const rows: ReadonlyArray<CommandRow> = await this.sql`
      SELECT command_ref, sandbox_ref, owner_user_id, tenant_ref,
             idempotency_ref, command_fingerprint, settlement_fingerprint, command_json,
             resource_generation, claimed_version, status, receipt_ref
      FROM khala_sync_managed_sandbox_commands
      WHERE command_ref = ${commandRef}
      LIMIT 1
    `;
    const row = rows[0];
    if (row === undefined) return undefined;
    if (row.owner_user_id !== ownerRef || row.tenant_ref !== tenantRef) {
      throw new ManagedSandboxStoreError("permission_denied", "command scope does not match");
    }
    const resourceRow = await selectResource(this.sql, row.sandbox_ref);
    if (resourceRow === undefined) {
      throw new ManagedSandboxStoreError("corrupt_store", "command lost its managed sandbox");
    }
    const command = commandFromRow(row);
    return {
      disposition: "replayed",
      status: row.status,
      command,
      resource: resourceFromRow(resourceRow),
      receipt: await receiptForCommand(this.sql, commandRef),
      turnSequence: await turnForCommand(this.sql, commandRef),
    };
  }

  async reserve(raw: ReserveManagedSandboxCommandInput): Promise<ManagedSandboxCommandReservation> {
    const command = publicSafe(decodeCommand(raw.command));
    const commandFingerprint = fingerprint(command);
    const createResource =
      command._tag === "Create" ? validateNewResource(command, raw.initialResource) : undefined;
    const sandboxRef = command._tag === "Create" ? createResource!.sandboxRef : command.sandboxRef;

    return this.sql.begin(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${sandboxRef}, 0))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${command.ownerRef}|${command.tenantRef}|${command.idempotencyRef}`}, 1))`;
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${command.commandRef}, 2))`;

      const existing = await selectCommand(
        tx,
        {
          commandRef: command.commandRef,
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          idempotencyRef: command.idempotencyRef,
        },
        true,
      );
      if (existing !== undefined) {
        return reservationReplay(tx, existing, commandFingerprint, sandboxRef);
      }

      let resource: ManagedSandboxResource;
      let row = await selectResource(tx, sandboxRef, true);
      if (command._tag === "Create") {
        if (row !== undefined) {
          throw new ManagedSandboxStoreError(
            "command_conflict",
            "managed sandbox ref already exists under another create command",
          );
        }
        resource = createResource!;
        await tx`
          INSERT INTO khala_sync_managed_sandboxes
            (sandbox_ref, owner_user_id, tenant_ref, program_ref, work_unit_ref,
             attachment_ref, attachment_generation, resource_generation, version,
             last_event_sequence, target_ref, lifecycle, lease_state,
             lease_expires_at, guest_state, filesystem_state, ingress_state,
             runtime_state, accepting_work, cleanup_complete, active_command_ref,
             resource_json, created_at, updated_at)
          VALUES
            (${resource.sandboxRef}, ${resource.ownerRef}, ${resource.tenantRef},
             ${resource.programRef}, ${resource.workUnitRef}, ${resource.attachmentRef},
             ${resource.attachmentGeneration}, ${resource.resourceGeneration},
             ${resource.version}, ${resource.lastEventSequence}, ${resource.target.targetRef},
             ${resource.facts.lifecycle}, ${resource.facts.leaseState},
             ${resource.lease.expiresAt}, ${resource.facts.guestState},
             ${resource.facts.filesystemState}, ${resource.facts.ingressState},
             ${resource.facts.runtimeState}, ${resource.facts.acceptingWork},
             ${resource.facts.cleanupComplete}, ${command.commandRef},
             ${resource}::jsonb, ${resource.createdAt}, ${resource.updatedAt})
        `;
        await tx`
          INSERT INTO khala_sync_managed_sandbox_generations
            (sandbox_ref, resource_generation, lifecycle, accepting_work, opened_at)
          VALUES
            (${sandboxRef}, ${resource.resourceGeneration}, ${resource.facts.lifecycle},
             FALSE, ${resource.createdAt})
        `;
        row = await selectResource(tx, sandboxRef, true);
      } else {
        if (row === undefined) {
          throw new ManagedSandboxStoreError("not_found", "managed sandbox does not exist");
        }
        assertScope(row, command.ownerRef, command.tenantRef);
        if (row.active_command_ref !== null) {
          throw new ManagedSandboxStoreError(
            "command_conflict",
            "another managed sandbox command is pending",
          );
        }
        resource = resourceFromRow(row);
        assertExpectedVersion(command, resource);
        if (
          command._tag === "Update" &&
          command.lease === undefined &&
          command.budget === undefined
        ) {
          throw new ManagedSandboxStoreError(
            "invalid",
            "update requires a lease or budget replacement",
          );
        }
      }

      const claimedVersion = resource.version;
      await tx`
        INSERT INTO khala_sync_managed_sandbox_commands
          (command_ref, sandbox_ref, owner_user_id, tenant_ref, idempotency_ref,
           command_kind, command_fingerprint, command_json, resource_generation,
           claimed_version, status, created_at, updated_at)
        VALUES
          (${command.commandRef}, ${sandboxRef}, ${command.ownerRef}, ${command.tenantRef},
           ${command.idempotencyRef}, ${command._tag}, ${commandFingerprint},
           ${command}::jsonb, ${resource.resourceGeneration},
           ${claimedVersion}, 'pending', ${command.requestedAt}, ${command.requestedAt})
      `;

      if (command._tag === "Inspect") {
        return settledCommand(tx, command, resource, "settled", "succeeded");
      }

      if (command._tag === "Update") {
        resource = validateResourceBounds(
          decodeResource({
            ...resource,
            version: resource.version + 1,
            ...(command.lease === undefined
              ? {}
              : {
                  lease: command.lease,
                  facts: { ...resource.facts, leaseState: command.lease.state },
                }),
            ...(command.budget === undefined ? {} : { budget: command.budget }),
            updatedAt: command.requestedAt,
          }),
        );
        await updateResource(tx, resource, null);
        return settledCommand(tx, command, resource, "settled", "succeeded");
      }

      const intentEvent = makeIntentEvent(command, resource);
      if (intentEvent !== null) {
        const model = applyEvents(resource, command, [intentEvent]);
        resource = withModel(resource, model, resource.version + 1, command.requestedAt);
        await tx`
          INSERT INTO khala_sync_managed_sandbox_events
            (sandbox_ref, sequence, event_ref, command_ref, resource_generation,
             event_kind, event_json, observed_at)
          VALUES
            (${sandboxRef}, ${intentEvent.sequence}, ${intentEvent.eventRef},
             ${command.commandRef}, ${intentEvent.resourceGeneration}, ${intentEvent._tag},
             ${intentEvent}::jsonb, ${intentEvent.observedAt})
        `;
      } else {
        resource = decodeResource({
          ...resource,
          version: resource.version + 1,
          updatedAt: command.requestedAt,
        });
      }

      let turnSequence: number | undefined;
      if (command._tag === "Dispatch") {
        const sequenceRows: ReadonlyArray<{ next_turn_sequence: string | number }> = await tx`
          SELECT next_turn_sequence
          FROM khala_sync_managed_sandboxes
          WHERE sandbox_ref = ${sandboxRef}
          FOR UPDATE
        `;
        turnSequence = integer(sequenceRows[0]!.next_turn_sequence, "next turn sequence");
        const turn = decodeTurn({
          schema: "openagents.managed_sandbox_turn.v1",
          turnRef: command.turnRef,
          sandboxRef,
          ownerRef: resource.ownerRef,
          tenantRef: resource.tenantRef,
          workUnitRef: resource.workUnitRef,
          attachmentRef: resource.attachmentRef,
          attachmentGeneration: resource.attachmentGeneration,
          resourceGeneration: resource.resourceGeneration,
          turnSequence,
          lastEventSequence: 0,
          commandRef: command.commandRef,
          capabilityRef: command.capabilityRef,
          promptDigest: command.promptDigest,
          runtime: command.runtime,
          status: "pending",
          createdAt: command.requestedAt,
        });
        await tx`
          INSERT INTO khala_sync_managed_sandbox_turns
            (sandbox_ref, turn_sequence, turn_ref, command_ref, resource_generation,
             status, provider, model_ref, harness_ref, reasoning_effort,
             prompt_digest, last_event_sequence, turn_json, created_at, updated_at)
          VALUES
            (${sandboxRef}, ${turnSequence}, ${command.turnRef}, ${command.commandRef},
             ${resource.resourceGeneration}, 'pending', ${command.runtime.provider},
             ${command.runtime.modelRef}, ${command.runtime.harnessRef},
             ${command.runtime.reasoningEffort ?? null}, ${command.promptDigest}, 0,
             ${turn}::jsonb, ${command.requestedAt}, ${command.requestedAt})
        `;
        await tx`
          UPDATE khala_sync_managed_sandboxes
          SET next_turn_sequence = next_turn_sequence + 1
          WHERE sandbox_ref = ${sandboxRef}
        `;
      }

      await updateResource(tx, resource, command.commandRef);
      await tx`
        UPDATE khala_sync_managed_sandbox_generations
        SET lifecycle = ${resource.facts.lifecycle},
            accepting_work = ${resource.facts.acceptingWork},
            fenced_at = CASE
              WHEN ${resource.facts.acceptingWork} THEN NULL
              ELSE ${resource.updatedAt}::timestamptz
            END
        WHERE sandbox_ref = ${sandboxRef}
          AND resource_generation = ${resource.resourceGeneration}
      `;
      return {
        disposition: "reserved",
        status: "pending",
        command,
        resource,
        turnSequence,
      };
    });
  }

  async settle(raw: SettleManagedSandboxCommandInput): Promise<ManagedSandboxReceipt> {
    const ownerRef = decodeRef(raw.ownerRef);
    const tenantRef = decodeRef(raw.tenantRef);
    const sandboxRef = decodeRef(raw.sandboxRef);
    const commandRef = decodeRef(raw.commandRef);
    const observedAt = S.decodeUnknownSync(
      S.String.check(S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/)),
    )(raw.observedAt);
    const events = raw.events.map((item) => publicSafe(decodeEvent(item)));
    const artifactRefs = (raw.artifactRefs ?? []).map((value) => decodeRef(value));
    const errorCode = raw.errorCode === undefined ? undefined : decodeRef(raw.errorCode);
    const settlementFingerprint = fingerprint({
      expectedResourceGeneration: raw.expectedResourceGeneration,
      events,
      outcome: raw.outcome,
      artifactRefs,
      errorCode,
      observedAt,
    });
    if (events.length === 0) {
      throw new ManagedSandboxStoreError("invalid", "settlement requires native events");
    }

    return this.sql.begin(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${sandboxRef}, 0))`;
      const resourceRow = await selectResource(tx, sandboxRef, true);
      if (resourceRow === undefined) {
        throw new ManagedSandboxStoreError("not_found", "managed sandbox does not exist");
      }
      assertScope(resourceRow, ownerRef, tenantRef);
      const commandRows: ReadonlyArray<CommandRow> = await tx`
        SELECT command_ref, sandbox_ref, owner_user_id, tenant_ref,
               idempotency_ref, command_fingerprint, settlement_fingerprint, command_json,
               resource_generation, claimed_version, status, receipt_ref
        FROM khala_sync_managed_sandbox_commands
        WHERE command_ref = ${commandRef}
        FOR UPDATE
      `;
      const commandRow = commandRows[0];
      if (commandRow === undefined || commandRow.sandbox_ref !== sandboxRef) {
        throw new ManagedSandboxStoreError("not_found", "managed sandbox command does not exist");
      }
      if (commandRow.owner_user_id !== ownerRef || commandRow.tenant_ref !== tenantRef) {
        throw new ManagedSandboxStoreError("permission_denied", "command scope does not match");
      }
      const existingReceipt = await receiptForCommand(tx, commandRef);
      if (existingReceipt !== undefined) {
        if (commandRow.settlement_fingerprint !== settlementFingerprint) {
          throw new ManagedSandboxStoreError(
            "event_conflict",
            "settled command is bound to different event or receipt bytes",
          );
        }
        return existingReceipt;
      }
      if (commandRow.status !== "pending" || resourceRow.active_command_ref !== commandRef) {
        throw new ManagedSandboxStoreError("command_conflict", "command no longer owns settlement");
      }
      const resource = resourceFromRow(resourceRow);
      if (resource.resourceGeneration !== raw.expectedResourceGeneration) {
        throw new ManagedSandboxStoreError(
          "stale_generation",
          `expected generation ${resource.resourceGeneration}, received ${raw.expectedResourceGeneration}`,
        );
      }
      const command = commandFromRow(commandRow);
      const model = applyEvents(resource, command, events);
      if (
        raw.outcome === "succeeded" &&
        ["failed", "recovery_required"].includes(model.lifecycle)
      ) {
        throw new ManagedSandboxStoreError(
          "invalid",
          "failed or recovery-required state cannot receive a succeeded receipt",
        );
      }

      if (model.resourceGeneration !== resource.resourceGeneration) {
        await tx`
          UPDATE khala_sync_managed_sandbox_generations
          SET accepting_work = FALSE, lifecycle = ${resource.facts.lifecycle},
              fenced_at = ${observedAt}
          WHERE sandbox_ref = ${sandboxRef}
            AND resource_generation = ${resource.resourceGeneration}
        `;
        await tx`
          INSERT INTO khala_sync_managed_sandbox_generations
            (sandbox_ref, resource_generation, lifecycle, accepting_work, opened_at)
          VALUES
            (${sandboxRef}, ${model.resourceGeneration}, ${model.lifecycle}, FALSE, ${observedAt})
        `;
      }

      for (const event of events) {
        const eventTurnRef = "turnRef" in event ? event.turnRef : null;
        const turnEventSequence = "turnEventSequence" in event ? event.turnEventSequence : null;
        await tx`
          INSERT INTO khala_sync_managed_sandbox_events
            (sandbox_ref, sequence, event_ref, command_ref, resource_generation,
             event_kind, event_json, observed_at, turn_ref, turn_event_sequence)
          VALUES
            (${sandboxRef}, ${event.sequence}, ${event.eventRef}, ${commandRef},
             ${event.resourceGeneration}, ${event._tag}, ${event}::jsonb,
             ${event.observedAt}, ${eventTurnRef}, ${turnEventSequence})
        `;
      }

      const nextResource = withModel(resource, model, resource.version + 1, observedAt);
      await updateResource(tx, nextResource, null);
      await tx`
        UPDATE khala_sync_managed_sandbox_generations
        SET lifecycle = ${nextResource.facts.lifecycle},
            accepting_work = ${nextResource.facts.acceptingWork},
            fenced_at = CASE
              WHEN ${nextResource.facts.acceptingWork} THEN NULL
              ELSE ${observedAt}::timestamptz
            END
        WHERE sandbox_ref = ${sandboxRef}
          AND resource_generation = ${nextResource.resourceGeneration}
      `;

      const receipt = makeReceipt(
        command,
        nextResource,
        raw.outcome,
        events.map((event) => event.eventRef),
        artifactRefs,
        errorCode,
        observedAt,
      );
      await insertReceipt(tx, receipt);
      const status =
        nextResource.facts.lifecycle === "recovery_required"
          ? "recovery_required"
          : raw.outcome === "refused"
            ? "refused"
            : "settled";
      await tx`
        UPDATE khala_sync_managed_sandbox_commands
        SET status = ${status}, receipt_ref = ${receipt.receiptRef},
            settlement_fingerprint = ${settlementFingerprint}, updated_at = ${observedAt}
        WHERE command_ref = ${commandRef}
      `;
      if (command._tag === "Dispatch") {
        const row = await selectRuntimeTurn(tx, sandboxRef, command.turnRef, true);
        if (row === undefined) {
          throw new ManagedSandboxStoreError("corrupt_store", "dispatch lost its runtime turn");
        }
        const turn = advanceTurn(turnFromRow(row), events);
        const turnReceipt = makeTurnReceipt(turn, turn.lastEventSequence, artifactRefs);
        await tx`
          UPDATE khala_sync_managed_sandbox_turns
          SET status = ${turn.status}, last_event_sequence = ${turn.lastEventSequence},
              turn_json = ${turn}::jsonb,
              turn_receipt_json = ${turnReceipt ?? null}::jsonb,
              updated_at = ${observedAt}
          WHERE command_ref = ${commandRef}
        `;
      }
      if (command._tag === "Interrupt") {
        const row = await selectRuntimeTurn(tx, sandboxRef, command.turnRef, true);
        if (row === undefined) {
          throw new ManagedSandboxStoreError("not_found", "interrupt target turn does not exist");
        }
        const turn = advanceTurn(turnFromRow(row), events);
        const turnReceipt = makeTurnReceipt(turn, turn.lastEventSequence, artifactRefs);
        await tx`
          UPDATE khala_sync_managed_sandbox_turns
          SET status = ${turn.status}, last_event_sequence = ${turn.lastEventSequence},
              interrupt_command_ref = ${commandRef}, turn_json = ${turn}::jsonb,
              turn_receipt_json = ${turnReceipt ?? null}::jsonb,
              updated_at = ${observedAt}
          WHERE sandbox_ref = ${sandboxRef} AND turn_ref = ${command.turnRef}
        `;
      }
      return receipt;
    });
  }

  async inspect(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
    }>,
  ): Promise<ManagedSandboxResource> {
    const row = await selectResource(this.sql, decodeRef(input.sandboxRef));
    if (row === undefined) {
      throw new ManagedSandboxStoreError("not_found", "managed sandbox does not exist");
    }
    assertScope(row, decodeRef(input.ownerRef), decodeRef(input.tenantRef));
    return resourceFromRow(row);
  }

  async list(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      limit?: number | undefined;
    }>,
  ): Promise<ReadonlyArray<ManagedSandboxResource>> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const limit = Math.max(1, Math.min(input.limit ?? 100, 200));
    const rows: ReadonlyArray<ResourceRow> = await this.sql`
      SELECT sandbox_ref, owner_user_id, tenant_ref, resource_generation,
             version, last_event_sequence, active_command_ref, resource_json
      FROM khala_sync_managed_sandboxes
      WHERE owner_user_id = ${ownerRef} AND tenant_ref = ${tenantRef}
      ORDER BY updated_at DESC, sandbox_ref ASC
      LIMIT ${limit}
    `;
    return rows.map(resourceFromRow);
  }

  async readEvents(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
      afterSequence: number;
      limit: number;
    }>,
  ): Promise<ManagedSandboxEventPage> {
    const resource = await this.inspect(input);
    if (
      !Number.isSafeInteger(input.afterSequence) ||
      input.afterSequence < 0 ||
      input.afterSequence > resource.lastEventSequence ||
      !Number.isSafeInteger(input.limit) ||
      input.limit < 1 ||
      input.limit > 1_000
    ) {
      throw new ManagedSandboxStoreError("cursor_conflict", "native event cursor is invalid");
    }
    const rows: ReadonlyArray<EventRow> = await this.sql`
      SELECT event_json
      FROM khala_sync_managed_sandbox_events
      WHERE sandbox_ref = ${resource.sandboxRef} AND sequence > ${input.afterSequence}
      ORDER BY sequence ASC
      LIMIT ${input.limit}
    `;
    const events = rows.map(eventFromRow);
    return {
      sandboxRef: resource.sandboxRef,
      afterSequence: input.afterSequence,
      nextSequence: events.at(-1)?.sequence ?? input.afterSequence,
      terminalSequence: resource.lastEventSequence,
      events,
    };
  }

  async inspectTurn(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
      turnRef: string;
    }>,
  ): Promise<Readonly<{ turn: ManagedSandboxTurn; receipt?: ManagedSandboxTurnReceipt }>> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const sandboxRef = decodeRef(input.sandboxRef);
    const turnRef = decodeRef(input.turnRef);
    const resource = await this.inspect({ ownerRef, tenantRef, sandboxRef });
    const row = await selectRuntimeTurn(this.sql, resource.sandboxRef, turnRef);
    if (row === undefined) {
      throw new ManagedSandboxStoreError(
        "not_found",
        "managed sandbox runtime turn does not exist",
      );
    }
    const turn = turnFromRow(row);
    if (turn.ownerRef !== ownerRef || turn.tenantRef !== tenantRef) {
      throw new ManagedSandboxStoreError("permission_denied", "runtime turn scope does not match");
    }
    const receipt = turnReceiptFromRow(row);
    return receipt === undefined ? { turn } : { turn, receipt };
  }

  async readTurnEvents(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
      turnRef: string;
      afterTurnSequence: number;
      limit: number;
    }>,
  ): Promise<ManagedSandboxRuntimeEventPage> {
    const { turn } = await this.inspectTurn(input);
    if (
      !Number.isSafeInteger(input.afterTurnSequence) ||
      input.afterTurnSequence < 0 ||
      input.afterTurnSequence > turn.lastEventSequence ||
      !Number.isSafeInteger(input.limit) ||
      input.limit < 1 ||
      input.limit > 1_000
    ) {
      throw new ManagedSandboxStoreError("cursor_conflict", "runtime turn cursor is invalid");
    }
    const rows: ReadonlyArray<EventRow> = await this.sql`
      SELECT event_json
      FROM khala_sync_managed_sandbox_events
      WHERE sandbox_ref = ${turn.sandboxRef}
        AND turn_ref = ${turn.turnRef}
        AND turn_event_sequence > ${input.afterTurnSequence}
      ORDER BY turn_event_sequence ASC
      LIMIT ${input.limit}
    `;
    const events = rows.map(eventFromRow);
    return {
      turn,
      events,
      afterTurnSequence: input.afterTurnSequence,
      nextTurnSequence:
        events.length === 0
          ? input.afterTurnSequence
          : (events.at(-1)! as ManagedSandboxEvent & { readonly turnEventSequence: number })
              .turnEventSequence,
      terminalTurnSequence: turn.lastEventSequence,
    };
  }

  async recordRuntimeEvents(
    raw: RecordManagedSandboxRuntimeEventsInput,
  ): Promise<RecordManagedSandboxRuntimeEventsResult> {
    const ownerRef = decodeRef(raw.ownerRef);
    const tenantRef = decodeRef(raw.tenantRef);
    const sandboxRef = decodeRef(raw.sandboxRef);
    const turnRef = decodeRef(raw.turnRef);
    const events = raw.events.map((item) => publicSafe(decodeRuntimeEventInput(item)));
    const evidenceRefs = (raw.evidenceRefs ?? []).map((value) => decodeRef(value));
    if (
      !Number.isSafeInteger(raw.expectedResourceGeneration) ||
      raw.expectedResourceGeneration < 0
    ) {
      throw new ManagedSandboxStoreError("invalid", "runtime event generation is invalid");
    }
    if (
      events.some(
        (event) =>
          event.turnRef !== turnRef || event.resourceGeneration !== raw.expectedResourceGeneration,
      )
    ) {
      throw new ManagedSandboxStoreError(
        "stale_generation",
        "runtime events do not bind the exact turn and resource generation",
      );
    }
    for (let index = 1; index < events.length; index += 1) {
      if (events[index]!.turnEventSequence !== events[index - 1]!.turnEventSequence + 1) {
        throw new ManagedSandboxStoreError("event_conflict", "runtime event page is not dense");
      }
    }

    return this.sql.begin(async (tx) => {
      await tx`SELECT pg_advisory_xact_lock(hashtextextended(${sandboxRef}, 0))`;
      const resourceRow = await selectResource(tx, sandboxRef, true);
      if (resourceRow === undefined) {
        throw new ManagedSandboxStoreError("not_found", "managed sandbox does not exist");
      }
      assertScope(resourceRow, ownerRef, tenantRef);
      let resource = resourceFromRow(resourceRow);
      if (resource.resourceGeneration !== raw.expectedResourceGeneration) {
        throw new ManagedSandboxStoreError(
          "stale_generation",
          `expected generation ${resource.resourceGeneration}, received ${raw.expectedResourceGeneration}`,
        );
      }
      const turnRow = await selectRuntimeTurn(tx, sandboxRef, turnRef, true);
      if (turnRow === undefined) {
        throw new ManagedSandboxStoreError(
          "not_found",
          "managed sandbox runtime turn does not exist",
        );
      }
      let turn = turnFromRow(turnRow);
      if (turn.ownerRef !== ownerRef || turn.tenantRef !== tenantRef) {
        throw new ManagedSandboxStoreError(
          "permission_denied",
          "runtime turn scope does not match",
        );
      }
      if (turn.resourceGeneration !== raw.expectedResourceGeneration) {
        throw new ManagedSandboxStoreError(
          "stale_generation",
          "runtime turn belongs to another resource generation",
        );
      }
      const commandRows: ReadonlyArray<CommandRow> = await tx`
        SELECT command_ref, sandbox_ref, owner_user_id, tenant_ref,
               idempotency_ref, command_fingerprint, settlement_fingerprint, command_json,
               resource_generation, claimed_version, status, receipt_ref
        FROM khala_sync_managed_sandbox_commands
        WHERE command_ref = ${turn.commandRef}
        FOR UPDATE
      `;
      const commandRow = commandRows[0];
      if (commandRow === undefined) {
        throw new ManagedSandboxStoreError(
          "corrupt_store",
          "runtime turn lost its dispatch command",
        );
      }
      const command = commandFromRow(commandRow);
      if (command._tag !== "Dispatch") {
        throw new ManagedSandboxStoreError("corrupt_store", "runtime turn command is not dispatch");
      }

      const appended: Array<ManagedSandboxEvent> = [];
      const initialResourceVersion = resource.version;
      for (const input of events) {
        if (input.turnEventSequence <= turn.lastEventSequence) {
          const rows: ReadonlyArray<EventRow> = await tx`
            SELECT event_json
            FROM khala_sync_managed_sandbox_events
            WHERE sandbox_ref = ${sandboxRef}
              AND turn_ref = ${turnRef}
              AND turn_event_sequence = ${input.turnEventSequence}
            FOR UPDATE
          `;
          const existing = rows[0];
          if (
            existing === undefined ||
            !same(runtimeInputFromEvent(eventFromRow(existing)), input)
          ) {
            throw new ManagedSandboxStoreError(
              "event_conflict",
              "runtime event sequence is bound to different provider bytes",
            );
          }
          continue;
        }
        if (input.turnEventSequence !== turn.lastEventSequence + 1) {
          throw new ManagedSandboxStoreError("event_conflict", "runtime event sequence has a gap");
        }
        const event = materializeRuntimeEvent(input, sandboxRef, resource.lastEventSequence + 1);
        const model = applyEvents(resource, command, [event]);
        resource = withModel(resource, model, resource.version, event.observedAt);
        turn = advanceTurn(turn, [event]);
        const causalCommandRef =
          event._tag === "RuntimeInterrupted" && turnRow.interrupt_command_ref !== null
            ? turnRow.interrupt_command_ref
            : turn.commandRef;
        await tx`
          INSERT INTO khala_sync_managed_sandbox_events
            (sandbox_ref, sequence, event_ref, command_ref, resource_generation,
             event_kind, event_json, observed_at, turn_ref, turn_event_sequence)
          VALUES
            (${sandboxRef}, ${event.sequence}, ${event.eventRef}, ${causalCommandRef},
             ${event.resourceGeneration}, ${event._tag}, ${event}::jsonb,
             ${event.observedAt}, ${turnRef}, ${input.turnEventSequence})
        `;
        appended.push(event);
      }

      let receipt = turnReceiptFromRow(turnRow);
      if (appended.length > 0) {
        resource = decodeResource({ ...resource, version: initialResourceVersion + 1 });
        receipt = makeTurnReceipt(turn, turn.lastEventSequence, evidenceRefs);
        await updateResource(tx, resource, resourceRow.active_command_ref);
        await tx`
          UPDATE khala_sync_managed_sandbox_generations
          SET lifecycle = ${resource.facts.lifecycle},
              accepting_work = ${resource.facts.acceptingWork},
              fenced_at = CASE
                WHEN ${resource.facts.acceptingWork} THEN NULL
                ELSE ${resource.updatedAt}::timestamptz
              END
          WHERE sandbox_ref = ${sandboxRef}
            AND resource_generation = ${resource.resourceGeneration}
        `;
        await tx`
          UPDATE khala_sync_managed_sandbox_turns
          SET status = ${turn.status}, last_event_sequence = ${turn.lastEventSequence},
              turn_json = ${turn}::jsonb,
              turn_receipt_json = ${receipt ?? null}::jsonb,
              updated_at = ${resource.updatedAt}
          WHERE sandbox_ref = ${sandboxRef} AND turn_ref = ${turnRef}
        `;
      }
      return { turn, receipt, events: appended };
    });
  }

  async pending(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      limit?: number | undefined;
    }>,
  ): Promise<ReadonlyArray<ManagedSandboxPendingCommand>> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const limit = Math.max(1, Math.min(input.limit ?? 100, 200));
    const rows: ReadonlyArray<CommandRow & ResourceRow> = await this.sql`
      SELECT c.command_ref, c.sandbox_ref, c.owner_user_id, c.tenant_ref,
             c.idempotency_ref, c.command_fingerprint, c.settlement_fingerprint, c.command_json,
             c.resource_generation, c.claimed_version, c.status, c.receipt_ref,
             s.version, s.last_event_sequence, s.active_command_ref, s.resource_json
      FROM khala_sync_managed_sandbox_commands c
      JOIN khala_sync_managed_sandboxes s USING (sandbox_ref)
      WHERE c.owner_user_id = ${ownerRef} AND c.tenant_ref = ${tenantRef}
        AND c.status = 'pending'
      ORDER BY c.created_at ASC
      LIMIT ${limit}
    `;
    return rows.map((row) => ({
      command: commandFromRow(row),
      resource: resourceFromRow(row),
      claimedVersion: integer(row.claimed_version, "claimed version"),
      resourceGeneration: integer(row.resource_generation, "command generation"),
    }));
  }

  async turns(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
    }>,
  ): Promise<ReadonlyArray<ManagedSandboxTurnOrder>> {
    const resource = await this.inspect(input);
    const rows: ReadonlyArray<TurnRow> = await this.sql`
      SELECT turn_sequence, turn_ref, status
      FROM khala_sync_managed_sandbox_turns
      WHERE sandbox_ref = ${resource.sandboxRef}
      ORDER BY turn_sequence ASC
    `;
    return rows.map((row) => ({
      turnSequence: integer(row.turn_sequence, "turn sequence"),
      turnRef: row.turn_ref,
      status: row.status as ManagedSandboxTurnOrder["status"],
    }));
  }

  async expired(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      at: string;
      limit?: number | undefined;
    }>,
  ): Promise<ReadonlyArray<ManagedSandboxResource>> {
    const ownerRef = decodeRef(input.ownerRef);
    const tenantRef = decodeRef(input.tenantRef);
    const at = new Date(input.at);
    if (Number.isNaN(at.valueOf())) {
      throw new ManagedSandboxStoreError("invalid", "lease comparison time is invalid");
    }
    const limit = Math.max(1, Math.min(input.limit ?? 100, 200));
    const rows: ReadonlyArray<ResourceRow> = await this.sql`
      SELECT sandbox_ref, owner_user_id, tenant_ref, resource_generation,
             version, last_event_sequence, active_command_ref, resource_json
      FROM khala_sync_managed_sandboxes
      WHERE owner_user_id = ${ownerRef} AND tenant_ref = ${tenantRef}
        AND lease_expires_at <= ${at.toISOString()}
        AND lifecycle NOT IN ('deleted', 'failed')
      ORDER BY lease_expires_at ASC
      LIMIT ${limit}
    `;
    return rows.map(resourceFromRow);
  }

  async readProjection(
    input: Readonly<{
      ownerRef: string;
      tenantRef: string;
      sandboxRef: string;
      translatorRef: string;
    }>,
  ): Promise<ManagedSandboxProjectionState | undefined> {
    const resource = await this.inspect(input);
    const translatorRef = decodeRef(input.translatorRef);
    const rows: ReadonlyArray<ProjectionRow> = await this.sql`
      SELECT projection_version, native_event_sequence, cursor_json
      FROM khala_sync_managed_sandbox_projection_cursors
      WHERE sandbox_ref = ${resource.sandboxRef} AND translator_ref = ${translatorRef}
    `;
    const row = rows[0];
    return row === undefined
      ? undefined
      : {
          projectionVersion: integer(row.projection_version, "projection version"),
          cursor: publicSafe(decodeProjectionCursor(row.cursor_json)),
        };
  }

  async advanceProjection(
    raw: AdvanceManagedSandboxProjectionInput,
  ): Promise<ManagedSandboxProjectionState> {
    const ownerRef = decodeRef(raw.ownerRef);
    const tenantRef = decodeRef(raw.tenantRef);
    const sandboxRef = decodeRef(raw.sandboxRef);
    const cursor = publicSafe(decodeProjectionCursor(raw.cursor));
    const observedAt = new Date(raw.observedAt);
    if (
      !Number.isSafeInteger(raw.expectedProjectionVersion) ||
      raw.expectedProjectionVersion < 0 ||
      Number.isNaN(observedAt.valueOf())
    ) {
      throw new ManagedSandboxStoreError("invalid", "projection version or time is invalid");
    }
    return this.sql.begin(async (tx) => {
      const resourceRow = await selectResource(tx, sandboxRef, true);
      if (resourceRow === undefined) {
        throw new ManagedSandboxStoreError("not_found", "managed sandbox does not exist");
      }
      assertScope(resourceRow, ownerRef, tenantRef);
      const resource = resourceFromRow(resourceRow);
      if (cursor.nativeEventSequence > resource.lastEventSequence) {
        throw new ManagedSandboxStoreError(
          "cursor_conflict",
          "compatibility cursor cannot advance beyond native event authority",
        );
      }
      const rows: ReadonlyArray<ProjectionRow> = await tx`
        SELECT projection_version, native_event_sequence, cursor_json
        FROM khala_sync_managed_sandbox_projection_cursors
        WHERE sandbox_ref = ${sandboxRef} AND translator_ref = ${cursor.translatorRef}
        FOR UPDATE
      `;
      const existing = rows[0];
      const version =
        existing === undefined ? 0 : integer(existing.projection_version, "projection version");
      if (version !== raw.expectedProjectionVersion) {
        throw new ManagedSandboxStoreError("cursor_conflict", "projection version is stale");
      }
      if (
        existing !== undefined &&
        cursor.nativeEventSequence <
          integer(existing.native_event_sequence, "projected native sequence")
      ) {
        throw new ManagedSandboxStoreError(
          "cursor_conflict",
          "projection cursor cannot move backward",
        );
      }
      const next = version + 1;
      await tx`
        INSERT INTO khala_sync_managed_sandbox_projection_cursors
          (sandbox_ref, translator_ref, projection_version, native_event_sequence,
           cursor_json, updated_at)
        VALUES
          (${sandboxRef}, ${cursor.translatorRef}, ${next}, ${cursor.nativeEventSequence},
           ${cursor}::jsonb, ${observedAt.toISOString()})
        ON CONFLICT (sandbox_ref, translator_ref) DO UPDATE
        SET projection_version = EXCLUDED.projection_version,
            native_event_sequence = EXCLUDED.native_event_sequence,
            cursor_json = EXCLUDED.cursor_json,
            updated_at = EXCLUDED.updated_at
      `;
      return { projectionVersion: next, cursor };
    });
  }
}
