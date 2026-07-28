import { Schema as S } from "effect";

export const ISSUE31_HOST_ANNOUNCEMENT_SCHEMA =
  "openagents.omega.issue31.host_discovery.v1" as const;
export const ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2 =
  "openagents.omega.issue31.host_discovery.v2" as const;
export const ISSUE31_PAIRING_SCHEMA = "openagents.omega.issue31.pairing.v1" as const;
export const ISSUE31_COMMAND_SCHEMA = "openagents.omega.issue31.command.v1" as const;
export const ISSUE31_COMMAND_SCHEMA_V2 = "openagents.omega.issue31.command.v2" as const;

export const ISSUE31_HOST_ANNOUNCEMENT_KIND = 31_990 as const;
export const ISSUE31_PRIVATE_RUMOR_KIND = 14 as const;
export const ISSUE31_PRIVATE_SEAL_KIND = 13 as const;
export const ISSUE31_PRIVATE_GIFT_WRAP_KIND = 1_059 as const;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const PublicRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/),
);
const RelayUrl = S.String.check(
  S.isMinLength(6),
  S.isMaxLength(512),
  S.isPattern(/^wss?:\/\/[^\s]+$/),
);
const UnixSeconds = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const Generation = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const ConversationTag = S.String.check(S.isPattern(/^sarah\.[0-9a-f]{24}$/));
const AgentThreadRef = S.Union([
  PublicRef,
  S.String.check(S.isPattern(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/)),
]);

export const Issue31HostAnnouncementSchema = S.Struct({
  schema: S.Literal(ISSUE31_HOST_ANNOUNCEMENT_SCHEMA),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  sarahPublicKeyHex: Hex64,
  displayName: S.String.check(S.isMinLength(1), S.isMaxLength(80)),
  protocols: S.Array(S.Literals([ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA])).check(
    S.isMinLength(2),
    S.isMaxLength(2),
  ),
  relayUrls: S.Array(RelayUrl).check(S.isMinLength(1), S.isMaxLength(8)),
  generation: Generation,
  issuedAt: UnixSeconds,
  expiresAt: UnixSeconds,
});
export interface Issue31HostAnnouncement extends S.Schema.Type<
  typeof Issue31HostAnnouncementSchema
> {}

export const Issue31HostAnnouncementV2Schema = S.Struct({
  ...Issue31HostAnnouncementSchema.fields,
  schema: S.Literal(ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2),
  conversation: ConversationTag,
  protocols: S.Array(
    S.Literals([ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA, ISSUE31_COMMAND_SCHEMA_V2]),
  ).check(S.isMinLength(3), S.isMaxLength(3)),
});
export interface Issue31HostAnnouncementV2 extends S.Schema.Type<
  typeof Issue31HostAnnouncementV2Schema
> {}
export type Issue31AnyHostAnnouncement = Issue31HostAnnouncement | Issue31HostAnnouncementV2;

const PairingBase = {
  schema: S.Literal(ISSUE31_PAIRING_SCHEMA),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  issuedAt: UnixSeconds,
};

export const Issue31PairingScopeSchema = S.Literals([
  "observe_issue31",
  "send_message",
  "interrupt_turn",
  "control_full_auto",
  "request_provider_handoff",
  "act_in_community",
]);
export type Issue31PairingScope = S.Schema.Type<typeof Issue31PairingScopeSchema>;

export const Issue31PairingRequestSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("pairing_request"),
  pairingRequestRef: PublicRef,
  requestedScopes: S.Array(Issue31PairingScopeSchema).check(S.isMinLength(1), S.isMaxLength(6)),
  expiresAt: UnixSeconds,
});

export const Issue31PairingChallengeSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("pairing_challenge"),
  pairingChallengeRef: PublicRef,
  pairingRequestEventId: Hex64,
  challenge: Hex64,
  expiresAt: UnixSeconds,
});

export const Issue31PairingResponseSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("pairing_response"),
  pairingResponseRef: PublicRef,
  pairingChallengeEventId: Hex64,
  challenge: Hex64,
  expiresAt: UnixSeconds,
});

export const Issue31ScopedGrantSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("scoped_grant"),
  sarahPublicKeyHex: Hex64,
  pairingResponseEventId: Hex64,
  grantRef: PublicRef,
  generation: Generation,
  scopes: S.Array(Issue31PairingScopeSchema).check(S.isMinLength(1), S.isMaxLength(6)),
  expiresAt: UnixSeconds,
});

export const Issue31GrantRenewalSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("grant_renewal"),
  sarahPublicKeyHex: Hex64,
  grantRef: PublicRef,
  previousGrantEventId: Hex64,
  priorGeneration: Generation,
  generation: Generation,
  scopes: S.Array(Issue31PairingScopeSchema).check(S.isMinLength(1), S.isMaxLength(6)),
  expiresAt: UnixSeconds,
});

export const Issue31GrantRevocationSchema = S.Struct({
  ...PairingBase,
  recordType: S.Literal("grant_revocation"),
  sarahPublicKeyHex: Hex64,
  grantRef: PublicRef,
  generation: Generation,
  reasonRef: S.optionalKey(PublicRef),
});

export const Issue31PairingRecordSchema = S.Union([
  Issue31PairingRequestSchema,
  Issue31PairingChallengeSchema,
  Issue31PairingResponseSchema,
  Issue31ScopedGrantSchema,
  Issue31GrantRenewalSchema,
  Issue31GrantRevocationSchema,
]);
export type Issue31PairingRecord = S.Schema.Type<typeof Issue31PairingRecordSchema>;

export const Issue31CommandIntentSchema = S.Struct({
  schema: S.Literal(ISSUE31_COMMAND_SCHEMA),
  recordType: S.Literal("command_intent"),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  grantRef: PublicRef,
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
  argumentsRef: PublicRef,
  issuedAt: UnixSeconds,
  expiresAt: UnixSeconds,
});
export interface Issue31CommandIntent extends S.Schema.Type<typeof Issue31CommandIntentSchema> {}

export const Issue31CommandResultSchema = S.Struct({
  schema: S.Literal(ISSUE31_COMMAND_SCHEMA),
  recordType: S.Literal("command_result"),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  grantRef: PublicRef,
  intentEventId: Hex64,
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
  status: S.Literals(["completed", "failed", "refused", "stopped", "unavailable"]),
  outcomeRef: PublicRef,
  reasonRef: S.optionalKey(PublicRef),
  completedAt: UnixSeconds,
});
export interface Issue31CommandResult extends S.Schema.Type<typeof Issue31CommandResultSchema> {}

const BoundedMessage = S.String.check(S.isMinLength(1), S.isMaxLength(12_000));
const BoundedNote = S.String.check(S.isMaxLength(4_096));
const ReminderId = S.String.check(S.isPattern(/^[0-9a-f]{32}$/));
const ReadStateSlotId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const ReadStateClientId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(64),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const ReadStateContextRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[^\u0000-\u001f\u007f]+$/),
);

export const ISSUE31_COMMAND_V2_ACTION_REFS = {
  sendMessage: "action.issue31.sarah.send",
  interruptTurn: "action.issue31.sarah.interrupt",
  agentThreadMessage: "action.issue31.omega.agent_thread_message",
  advanceReadState: "action.issue31.read_state.advance",
  createReminder: "action.issue31.reminder.create",
  changeReminder: "action.issue31.reminder.change",
  completeReminder: "action.issue31.reminder.complete",
  cancelReminder: "action.issue31.reminder.cancel",
} as const;

const Issue31ReminderPendingPostImageSchema = {
  reminderId: ReminderId,
  note: S.optionalKey(BoundedNote),
  targetEventId: S.optionalKey(Hex64),
  notBefore: UnixSeconds,
  expiration: S.optionalKey(UnixSeconds),
};

export const Issue31CommandArgumentsSchema = S.Union([
  S.Struct({
    kind: S.Literal("send_message"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.sendMessage),
    conversation: ConversationTag,
    text: BoundedMessage,
  }),
  S.Struct({
    kind: S.Literal("interrupt_turn"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.interruptTurn),
    conversation: ConversationTag,
    turnRef: PublicRef,
  }),
  S.Struct({
    kind: S.Literal("agent_thread_message"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.agentThreadMessage),
    threadRef: AgentThreadRef,
    text: BoundedMessage,
    disposition: S.Literals(["enqueue", "steer"]),
  }),
  S.Struct({
    kind: S.Literal("read_state_patch"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.advanceReadState),
    slotId: ReadStateSlotId,
    clientId: ReadStateClientId,
    contextRef: ReadStateContextRef,
    readAt: UnixSeconds,
  }),
  S.Struct({
    kind: S.Literal("reminder_create"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.createReminder),
    ...Issue31ReminderPendingPostImageSchema,
  }),
  S.Struct({
    kind: S.Literal("reminder_change"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.changeReminder),
    ...Issue31ReminderPendingPostImageSchema,
  }),
  S.Struct({
    kind: S.Literal("reminder_complete"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.completeReminder),
    reminderId: ReminderId,
  }),
  S.Struct({
    kind: S.Literal("reminder_cancel"),
    actionRef: S.Literal(ISSUE31_COMMAND_V2_ACTION_REFS.cancelReminder),
    reminderId: ReminderId,
  }),
]);
export type Issue31CommandArguments = S.Schema.Type<typeof Issue31CommandArgumentsSchema>;

export const Issue31CommandIntentV2Schema = S.Struct({
  schema: S.Literal(ISSUE31_COMMAND_SCHEMA_V2),
  recordType: S.Literal("command_intent"),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  grantRef: PublicRef,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
  arguments: Issue31CommandArgumentsSchema,
  issuedAt: UnixSeconds,
  expiresAt: UnixSeconds,
});
export interface Issue31CommandIntentV2 extends S.Schema.Type<
  typeof Issue31CommandIntentV2Schema
> {}

export const Issue31CommandResultV2Schema = S.Struct({
  schema: S.Literal(ISSUE31_COMMAND_SCHEMA_V2),
  recordType: S.Literal("command_result"),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  grantRef: PublicRef,
  intentEventId: Hex64,
  actionRef: PublicRef,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
  status: S.Literals(["accepted", "failed", "refused", "unavailable"]),
  handlingRef: PublicRef,
  reasonRef: S.optionalKey(PublicRef),
  sourceEventId: S.optionalKey(Hex64),
  handledAt: UnixSeconds,
});
export interface Issue31CommandResultV2 extends S.Schema.Type<
  typeof Issue31CommandResultV2Schema
> {}

export const Issue31CommandRecordV2Schema = S.Union([
  Issue31CommandIntentV2Schema,
  Issue31CommandResultV2Schema,
]);
export type Issue31CommandRecordV2 = S.Schema.Type<typeof Issue31CommandRecordV2Schema>;

export const Issue31CommandRecordSchema = S.Union([
  Issue31CommandIntentSchema,
  Issue31CommandResultSchema,
]);
export type Issue31CommandRecord = S.Schema.Type<typeof Issue31CommandRecordSchema>;

const decodeHostAnnouncement = S.decodeUnknownSync(Issue31HostAnnouncementSchema);
const decodeHostAnnouncementV2 = S.decodeUnknownSync(Issue31HostAnnouncementV2Schema);
const decodePairingRecord = S.decodeUnknownSync(Issue31PairingRecordSchema);
const decodeCommandRecord = S.decodeUnknownSync(Issue31CommandRecordSchema);
const decodeCommandRecordV2 = S.decodeUnknownSync(Issue31CommandRecordV2Schema);

const assertTimes = (issuedAt: number, expiresAt: number, label: string): void => {
  if (expiresAt <= issuedAt) throw new Error(`${label} expiration must follow issue time.`);
};

const assertUnique = (values: ReadonlyArray<string>, label: string): void => {
  if (new Set(values).size !== values.length) throw new Error(`${label} repeats a value.`);
};

const validateIssue31HostAnnouncement = <Announcement extends Issue31AnyHostAnnouncement>(
  record: Announcement,
): Announcement => {
  if (record.sarahPublicKeyHex === record.hostPublicKeyHex) {
    throw new Error("Device mirror host and Sarah signing keys must be distinct.");
  }
  assertTimes(record.issuedAt, record.expiresAt, "Device mirror host announcement");
  assertUnique(record.relayUrls, "Device mirror host announcement relay list");
  assertUnique(record.protocols, "Device mirror host announcement protocol list");
  if (
    !record.protocols.includes(ISSUE31_PAIRING_SCHEMA) ||
    !record.protocols.includes(ISSUE31_COMMAND_SCHEMA)
  ) {
    throw new Error("Device mirror host announcement must declare pairing and command protocols.");
  }
  if (
    record.schema === ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2 &&
    !record.protocols.includes(ISSUE31_COMMAND_SCHEMA_V2)
  ) {
    throw new Error("Device mirror v2 host announcement must declare command v2.");
  }
  for (const relayUrl of record.relayUrls) {
    const parsed = new URL(relayUrl);
    if (
      (parsed.protocol !== "wss:" && parsed.protocol !== "ws:") ||
      parsed.hostname === "" ||
      parsed.username !== "" ||
      parsed.password !== "" ||
      parsed.search !== "" ||
      parsed.hash !== ""
    ) {
      throw new Error("Device mirror host announcement contains an unsafe relay URL.");
    }
  }
  return record;
};

export const decodeIssue31HostAnnouncement = (value: unknown): Issue31HostAnnouncement =>
  validateIssue31HostAnnouncement(decodeHostAnnouncement(value, { onExcessProperty: "error" }));

export const decodeIssue31HostAnnouncementV2 = (value: unknown): Issue31HostAnnouncementV2 =>
  validateIssue31HostAnnouncement(decodeHostAnnouncementV2(value, { onExcessProperty: "error" }));

export const decodeIssue31AnyHostAnnouncement = (value: unknown): Issue31AnyHostAnnouncement => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Device mirror host announcement is not a record.");
  }
  return (value as Readonly<Record<string, unknown>>)["schema"] ===
    ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2
    ? decodeIssue31HostAnnouncementV2(value)
    : decodeIssue31HostAnnouncement(value);
};

export const decodeIssue31PairingRecord = (value: unknown): Issue31PairingRecord => {
  const record = decodePairingRecord(value, { onExcessProperty: "error" });
  if (record.recordType !== "grant_revocation") {
    assertTimes(record.issuedAt, record.expiresAt, `Device mirror ${record.recordType}`);
  }
  if (record.recordType === "pairing_request") {
    assertUnique(record.requestedScopes, "Device pairing request scopes");
  }
  if (record.recordType === "scoped_grant" || record.recordType === "grant_renewal") {
    assertUnique(record.scopes, `Device mirror ${record.recordType} scopes`);
  }
  if (
    (record.recordType === "scoped_grant" ||
      record.recordType === "grant_renewal" ||
      record.recordType === "grant_revocation") &&
    record.sarahPublicKeyHex === record.hostPublicKeyHex
  ) {
    throw new Error("Device mirror host and Sarah signing keys must be distinct.");
  }
  if (record.recordType === "grant_renewal") {
    if (record.generation !== record.priorGeneration + 1) {
      throw new Error("Device pairing grant renewal must advance one generation.");
    }
  }
  return record;
};

export const decodeIssue31CommandRecord = (value: unknown): Issue31CommandRecord => {
  const record = decodeCommandRecord(value, { onExcessProperty: "error" });
  if (record.recordType === "command_intent") {
    assertTimes(record.issuedAt, record.expiresAt, "Device mirror command intent");
  }
  return record;
};

export const decodeIssue31CommandRecordV2 = (value: unknown): Issue31CommandRecordV2 => {
  const record = decodeCommandRecordV2(value, { onExcessProperty: "error" });
  if (record.recordType === "command_intent") {
    assertTimes(record.issuedAt, record.expiresAt, "Device mirror command v2 intent");
    if (
      (record.arguments.kind === "reminder_create" ||
        record.arguments.kind === "reminder_change") &&
      record.arguments.expiration !== undefined &&
      record.arguments.expiration <= record.arguments.notBefore
    ) {
      throw new Error("Device mirror reminder expiration must follow not-before time.");
    }
    if (
      record.arguments.kind === "read_state_patch" &&
      new TextEncoder().encode(record.arguments.contextRef).length > 256
    ) {
      throw new Error("Device mirror read-state context reference exceeds 256 UTF-8 bytes.");
    }
  } else if (record.status === "accepted" && record.reasonRef !== undefined) {
    throw new Error("Device mirror accepted command handling cannot carry a failure reason.");
  }
  return record;
};

export const parseIssue31PrivateRumorContent = (
  content: string,
): Issue31PairingRecord | Issue31CommandRecord | Issue31CommandRecordV2 => {
  let value: unknown;
  try {
    value = JSON.parse(content);
  } catch {
    throw new Error("Device mirror private rumor content is not JSON.");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Device mirror private rumor content is not a record.");
  }
  const schema = (value as Readonly<Record<string, unknown>>)["schema"];
  if (schema === ISSUE31_PAIRING_SCHEMA) return decodeIssue31PairingRecord(value);
  if (schema === ISSUE31_COMMAND_SCHEMA) return decodeIssue31CommandRecord(value);
  if (schema === ISSUE31_COMMAND_SCHEMA_V2) return decodeIssue31CommandRecordV2(value);
  throw new Error("Device mirror private rumor has an unknown schema.");
};

export interface Issue31PairingEvent {
  readonly eventId: string;
  readonly record: Issue31PairingRecord;
}

type Issue31GrantLifecycleRecord = Extract<
  Issue31PairingRecord,
  Readonly<{
    recordType: "scoped_grant" | "grant_renewal" | "grant_revocation";
  }>
>;

const isIssue31GrantLifecycleRecord = (
  record: Issue31PairingRecord,
): record is Issue31GrantLifecycleRecord =>
  record.recordType === "scoped_grant" ||
  record.recordType === "grant_renewal" ||
  record.recordType === "grant_revocation";

export interface Issue31GrantState {
  readonly grantRef: string;
  readonly hostRef: string;
  readonly hostPublicKeyHex: string;
  readonly sarahPublicKeyHex: string;
  readonly devicePublicKeyHex: string;
  readonly generation: number;
  readonly status: "active" | "revoked";
  readonly scopes: ReadonlyArray<string>;
  readonly expiresAt: number | null;
  readonly issuedAt: number;
  readonly sourceEventId: string;
}

const grantFingerprint = (record: Issue31GrantLifecycleRecord): string =>
  JSON.stringify(record, Object.keys(record).sort());

const assertPairingIdentity = (
  expected: Readonly<{
    hostRef: string;
    hostPublicKeyHex: string;
    devicePublicKeyHex: string;
  }>,
  actual: Readonly<{
    hostRef: string;
    hostPublicKeyHex: string;
    devicePublicKeyHex: string;
  }>,
): void => {
  if (
    actual.hostRef !== expected.hostRef ||
    actual.hostPublicKeyHex !== expected.hostPublicKeyHex ||
    actual.devicePublicKeyHex !== expected.devicePublicKeyHex
  ) {
    throw new Error("Device pairing chain changes host or device identity.");
  }
};

const assertScopedGrantPairingChain = (
  grant: Extract<Issue31PairingRecord, Readonly<{ recordType: "scoped_grant" }>>,
  recordsByEventId: ReadonlyMap<string, Issue31PairingRecord>,
): void => {
  const response = recordsByEventId.get(grant.pairingResponseEventId);
  if (response?.recordType !== "pairing_response") {
    throw new Error("Device pairing grant has no pairing response.");
  }
  const challenge = recordsByEventId.get(response.pairingChallengeEventId);
  if (challenge?.recordType !== "pairing_challenge") {
    throw new Error("Device pairing response has no challenge.");
  }
  const request = recordsByEventId.get(challenge.pairingRequestEventId);
  if (request?.recordType !== "pairing_request") {
    throw new Error("Device pairing challenge has no request.");
  }
  assertPairingIdentity(grant, response);
  assertPairingIdentity(grant, challenge);
  assertPairingIdentity(grant, request);
  if (response.challenge !== challenge.challenge) {
    throw new Error("Device pairing response does not answer its challenge.");
  }
  if (grant.scopes.some((scope) => !request.requestedScopes.includes(scope))) {
    throw new Error("Device pairing grant exceeds the requested scopes.");
  }
  if (
    request.issuedAt > challenge.issuedAt ||
    challenge.issuedAt > response.issuedAt ||
    response.issuedAt > grant.issuedAt
  ) {
    throw new Error("Device pairing chain time order is invalid.");
  }
};

export const foldIssue31Grant = (
  events: ReadonlyArray<Issue31PairingEvent>,
  grantRef: string,
): Issue31GrantState | null => {
  const uniqueEvents = new Map<string, Issue31PairingRecord>();
  for (const event of events) {
    const prior = uniqueEvents.get(event.eventId);
    if (prior !== undefined && JSON.stringify(prior) !== JSON.stringify(event.record)) {
      throw new Error(`Device mirror event ${event.eventId} has conflicting records.`);
    }
    uniqueEvents.set(event.eventId, event.record);
  }
  const candidates: Array<readonly [string, Issue31GrantLifecycleRecord]> = [...uniqueEvents]
    .flatMap(([eventId, record]) =>
      isIssue31GrantLifecycleRecord(record) && record.grantRef === grantRef
        ? [[eventId, record] as const]
        : [],
    )
    .sort(([leftId, left], [rightId, right]) =>
      left.generation === right.generation
        ? leftId.localeCompare(rightId)
        : left.generation - right.generation,
    );
  const identity = candidates[0]?.[1];
  if (
    identity !== undefined &&
    candidates.some(
      ([, record]) =>
        record.hostRef !== identity.hostRef ||
        record.hostPublicKeyHex !== identity.hostPublicKeyHex ||
        record.sarahPublicKeyHex !== identity.sarahPublicKeyHex ||
        record.devicePublicKeyHex !== identity.devicePublicKeyHex,
    )
  ) {
    throw new Error(`Device pairing grant ${grantRef} has an identity fork.`);
  }
  const revocations = candidates.filter(([, record]) => record.recordType === "grant_revocation");
  const terminalRevocation = revocations.at(-1);
  if (terminalRevocation !== undefined) {
    const [eventId, record] = terminalRevocation;
    if (record.recordType !== "grant_revocation") {
      throw new Error("Device pairing grant revocation projection is invalid.");
    }
    return {
      grantRef,
      hostRef: record.hostRef,
      hostPublicKeyHex: record.hostPublicKeyHex,
      sarahPublicKeyHex: record.sarahPublicKeyHex,
      devicePublicKeyHex: record.devicePublicKeyHex,
      generation: record.generation,
      status: "revoked",
      scopes: [],
      expiresAt: null,
      issuedAt: record.issuedAt,
      sourceEventId: eventId,
    };
  }
  const recordsByGeneration = new Map<
    number,
    Array<readonly [string, Issue31GrantLifecycleRecord]>
  >();
  for (const candidate of candidates) {
    const sameGeneration = recordsByGeneration.get(candidate[1].generation) ?? [];
    sameGeneration.push(candidate);
    recordsByGeneration.set(candidate[1].generation, sameGeneration);
  }
  for (const [generation, sameGeneration] of recordsByGeneration) {
    if (
      sameGeneration.length > 1 &&
      new Set(sameGeneration.map(([, record]) => grantFingerprint(record))).size > 1
    ) {
      throw new Error(`Device pairing grant ${grantRef} forks at generation ${generation}.`);
    }
  }
  let state: Issue31GrantState | null = null;
  for (const [eventId, record] of candidates) {
    if (record.recordType === "grant_revocation") continue;
    if (state !== null) {
      if (record.recordType !== "grant_renewal") {
        throw new Error(`Device pairing grant ${grantRef} has more than one initial grant.`);
      }
      if (
        record.previousGrantEventId !== state.sourceEventId ||
        record.priorGeneration !== state.generation ||
        record.generation !== state.generation + 1 ||
        record.hostRef !== state.hostRef ||
        record.hostPublicKeyHex !== state.hostPublicKeyHex ||
        record.sarahPublicKeyHex !== state.sarahPublicKeyHex ||
        record.devicePublicKeyHex !== state.devicePublicKeyHex
      ) {
        throw new Error(`Device pairing grant ${grantRef} renewal lineage is invalid.`);
      }
    } else if (record.recordType !== "scoped_grant") {
      throw new Error(`Device pairing grant ${grantRef} renewal has no initial grant.`);
    }
    if (record.recordType === "scoped_grant") {
      assertScopedGrantPairingChain(record, uniqueEvents);
    }
    state = {
      grantRef,
      hostRef: record.hostRef,
      hostPublicKeyHex: record.hostPublicKeyHex,
      sarahPublicKeyHex: record.sarahPublicKeyHex,
      devicePublicKeyHex: record.devicePublicKeyHex,
      generation: record.generation,
      status: "active",
      scopes: record.scopes,
      expiresAt: record.expiresAt,
      issuedAt: record.issuedAt,
      sourceEventId: eventId,
    };
  }
  return state;
};

export interface Issue31CommandEvent {
  readonly eventId: string;
  readonly record: Issue31CommandRecord;
}

export interface Issue31CommandState {
  readonly intent: Issue31CommandIntent;
  readonly intentEventId: string;
  readonly result: Issue31CommandResult | null;
  readonly resultEventId: string | null;
}

const commandFingerprint = (intent: Issue31CommandIntent): string =>
  JSON.stringify([
    intent.hostRef,
    intent.hostPublicKeyHex,
    intent.devicePublicKeyHex,
    intent.grantRef,
    intent.actionRef,
    intent.expectedGeneration,
    intent.argumentsRef,
    intent.issuedAt,
    intent.expiresAt,
  ]);

const resultFingerprint = (result: Issue31CommandResult): string =>
  JSON.stringify([
    result.hostRef,
    result.hostPublicKeyHex,
    result.devicePublicKeyHex,
    result.grantRef,
    result.intentEventId,
    result.actionRef,
    result.idempotencyRef,
    result.expectedGeneration,
    result.status,
    result.outcomeRef,
    result.reasonRef ?? null,
    result.completedAt,
  ]);

export const reconcileIssue31Commands = (
  events: ReadonlyArray<Issue31CommandEvent>,
): ReadonlyArray<Issue31CommandState> => {
  const unique = new Map(events.map((event) => [event.eventId, event]));
  const intents = new Map<string, { eventId: string; record: Issue31CommandIntent }>();
  const results = new Map<string, { eventId: string; record: Issue31CommandResult }>();
  for (const event of unique.values()) {
    if (event.record.recordType === "command_intent") {
      const prior = intents.get(event.record.idempotencyRef);
      if (
        prior !== undefined &&
        commandFingerprint(prior.record) !== commandFingerprint(event.record)
      ) {
        throw new Error(`Device mirror idempotency conflict for ${event.record.idempotencyRef}.`);
      }
      if (prior === undefined || event.eventId < prior.eventId) {
        intents.set(event.record.idempotencyRef, { eventId: event.eventId, record: event.record });
      }
    } else {
      const prior = results.get(event.record.idempotencyRef);
      if (
        prior !== undefined &&
        resultFingerprint(prior.record) !== resultFingerprint(event.record)
      ) {
        throw new Error(`Device mirror terminal result conflict for ${event.record.idempotencyRef}.`);
      }
      if (prior === undefined || event.eventId < prior.eventId) {
        results.set(event.record.idempotencyRef, { eventId: event.eventId, record: event.record });
      }
    }
  }
  return [...intents]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([idempotencyRef, intent]) => {
      const result = results.get(idempotencyRef);
      if (result !== undefined) {
        if (
          result.record.intentEventId !== intent.eventId ||
          result.record.expectedGeneration !== intent.record.expectedGeneration ||
          result.record.actionRef !== intent.record.actionRef ||
          result.record.hostRef !== intent.record.hostRef ||
          result.record.hostPublicKeyHex !== intent.record.hostPublicKeyHex ||
          result.record.devicePublicKeyHex !== intent.record.devicePublicKeyHex ||
          result.record.grantRef !== intent.record.grantRef
        ) {
          throw new Error(`Device mirror command result does not match ${idempotencyRef}.`);
        }
      }
      return {
        intent: intent.record,
        intentEventId: intent.eventId,
        result: result?.record ?? null,
        resultEventId: result?.eventId ?? null,
      };
    });
};
