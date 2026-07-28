import { Schema as S } from "effect";

export const ISSUE31_HOST_ADJUNCT_SCHEMA = "openagents.omega.issue31.host.v1" as const;
export const MAX_ISSUE31_PROJECTION_REFS = 16 as const;
export const MAX_ISSUE31_TIMESTAMP_MS = 8_640_000_000_000_000 as const;

const PublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const TimestampMs = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(MAX_ISSUE31_TIMESTAMP_MS),
);
const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const Generation = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);

/**
 * How an adjunct names the host and the device it was delivered to (omega#49).
 *
 * An adjunct is a *statement about a host*, so on its own it names neither the
 * key that authored it nor the device it is for. Every other owner-private
 * Device mirror record does, and `assertRecordIdentity` in the private envelope
 * refuses one whose claims disagree with the signed seal author and the gift
 * wrap recipient. An adjunct that could not make those claims would have to be
 * waived through that check — which is exactly the binding that stops one host
 * addressing another host's device.
 *
 * So the adjunct states them itself. These fields are optional at the type
 * level because an adjunct that has not been delivered anywhere — one built by
 * the host for its own panels — has no device to name; the law below is
 * all-or-nothing, so a *partial* delivery claim is never representable.
 */
export const issue31AdjunctDeliveryFields = <RecordType extends string>(
  recordType: RecordType,
) =>
  ({
    /** Present exactly when this adjunct was delivered to a device. */
    recordType: S.optional(S.Literal(recordType)),
    /** The host key that must equal the signed seal author. */
    hostPublicKeyHex: S.optional(Hex64),
    /** The device key that must equal the gift wrap recipient. */
    devicePublicKeyHex: S.optional(Hex64),
    /** The scoped grant this delivery was made under. */
    grantRef: S.optional(PublicRef),
    /** The grant generation the host believed was current. */
    expectedGeneration: S.optional(Generation),
  }) as const;

export const ISSUE31_ADJUNCT_DELIVERY_KEYS = [
  "recordType",
  "hostPublicKeyHex",
  "devicePublicKeyHex",
  "grantRef",
  "expectedGeneration",
] as const;

/**
 * Refuse a half-stated delivery claim.
 *
 * A record carrying a device key but no host key, or a grant reference but no
 * device, would let a reader believe it had checked a binding that was never
 * made. Either every delivery field is present or none is.
 */
export const assertIssue31AdjunctDeliveryLaw = (
  adjunct: Readonly<Record<string, unknown>>,
): void => {
  const stated = ISSUE31_ADJUNCT_DELIVERY_KEYS.filter(
    (key) => adjunct[key] !== undefined,
  );
  if (stated.length !== 0 && stated.length !== ISSUE31_ADJUNCT_DELIVERY_KEYS.length) {
    throw new Error("Device mirror adjunct states a partial delivery binding.");
  }
};

export const Issue31ProjectionCapabilitySchema = S.Literals([
  "connection_identity",
  "full_auto_runs",
  "provider_accounts",
  "evidence_chain",
]);
export type Issue31ProjectionCapability = S.Schema.Type<typeof Issue31ProjectionCapabilitySchema>;

export const Issue31ProjectionSourceSchema = S.Struct({
  kind: S.Literal("omega_host"),
  sourceRef: PublicRef,
  observedAtMs: TimestampMs,
});
export interface Issue31ProjectionSource extends S.Schema.Type<
  typeof Issue31ProjectionSourceSchema
> {}

export const Issue31ProjectionFreshnessSchema = S.Literals(["current", "stale", "unknown"]);
export type Issue31ProjectionFreshness = S.Schema.Type<typeof Issue31ProjectionFreshnessSchema>;

export const Issue31GapSchema = S.Literals(["complete", "partial", "missing", "unavailable"]);
export type Issue31Gap = S.Schema.Type<typeof Issue31GapSchema>;

export const Issue31RoleSchema = S.Struct({
  kind: S.Literals(["owner", "member", "verifier", "observer"]),
  status: S.Literals(["active", "revoked", "unknown"]),
  grantRef: S.optional(PublicRef),
});
export interface Issue31Role extends S.Schema.Type<typeof Issue31RoleSchema> {}

export const Issue31CommandStateSchema = S.Union([
  S.Struct({ kind: S.Literal("idle") }),
  S.Struct({
    kind: S.Literal("pending"),
    intentRef: PublicRef,
    actionRef: PublicRef,
  }),
  S.Struct({
    kind: S.Literal("refused"),
    intentRef: PublicRef,
    actionRef: PublicRef,
    reasonClass: PublicRef,
    decisionRef: PublicRef,
    receiptRef: S.optional(PublicRef),
  }),
  S.Struct({
    kind: S.Literal("terminal"),
    intentRef: PublicRef,
    actionRef: PublicRef,
    state: S.Literals(["succeeded", "failed", "stopped", "unavailable"]),
    outcomeRef: PublicRef,
    reasonRef: S.optional(PublicRef),
    receiptRef: S.optional(PublicRef),
  }),
]);
export type Issue31CommandState = S.Schema.Type<typeof Issue31CommandStateSchema>;

export const Issue31HostProjectionSchema = S.Struct({
  capability: Issue31ProjectionCapabilitySchema,
  source: Issue31ProjectionSourceSchema,
  freshness: Issue31ProjectionFreshnessSchema,
  gap: Issue31GapSchema,
  role: Issue31RoleSchema,
  recordRefs: S.Array(PublicRef).check(S.isMaxLength(MAX_ISSUE31_PROJECTION_REFS)),
  permittedActionRefs: S.Array(PublicRef).check(S.isMaxLength(MAX_ISSUE31_PROJECTION_REFS)),
  commandState: Issue31CommandStateSchema,
});
export interface Issue31HostProjection extends S.Schema.Type<typeof Issue31HostProjectionSchema> {}

export const ISSUE31_HOST_ADJUNCT_RECORD_TYPE = "host_snapshot" as const;

export const Issue31HostAdjunctSchema = S.Struct({
  schema: S.Literal(ISSUE31_HOST_ADJUNCT_SCHEMA),
  hostRef: PublicRef,
  snapshotRef: PublicRef,
  generatedAtMs: TimestampMs,
  projections: S.Array(Issue31HostProjectionSchema).check(S.isMinLength(4), S.isMaxLength(4)),
  ...issue31AdjunctDeliveryFields(ISSUE31_HOST_ADJUNCT_RECORD_TYPE),
});
export interface Issue31HostAdjunct extends S.Schema.Type<typeof Issue31HostAdjunctSchema> {}

const forbiddenRefFragments = [
  "api_key",
  "apikey",
  "access_token",
  "refresh_token",
  "openagents_agent_token",
  "client_secret",
  "private_key",
] as const;

export const isIssue31PublicRef = (value: string): boolean => {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed.length > 256 || trimmed !== value) return false;
  if (!/^[A-Za-z0-9._:-]+$/.test(trimmed)) return false;
  const lower = trimmed.toLowerCase();
  if (
    lower.startsWith("sk-") ||
    lower.startsWith("sk_") ||
    lower.startsWith("ghp_") ||
    lower.startsWith("gho_") ||
    lower.startsWith("github_pat_") ||
    lower.startsWith("xox") ||
    lower.startsWith("nsec1") ||
    lower.startsWith("ncryptsec1") ||
    lower.includes("authorization:") ||
    lower.endsWith(".pem") ||
    lower.endsWith(".key") ||
    lower.includes("id_rsa") ||
    forbiddenRefFragments.some((fragment) => lower.includes(fragment))
  )
    return false;
  if (
    trimmed.length >= 40 &&
    !trimmed.includes(".") &&
    !trimmed.includes(":") &&
    /^[A-Za-z0-9+/_=-]+$/.test(trimmed)
  )
    return false;
  return (
    trimmed.includes(".") || trimmed.includes(":") || trimmed.includes("_") || trimmed.length <= 64
  );
};

const publicRefsForCommand = (command: Issue31CommandState): ReadonlyArray<string> => {
  if (command.kind === "idle") return [];
  if (command.kind === "pending") return [command.intentRef, command.actionRef];
  if (command.kind === "refused") {
    return [
      command.intentRef,
      command.actionRef,
      command.reasonClass,
      command.decisionRef,
      ...(command.receiptRef === undefined ? [] : [command.receiptRef]),
    ];
  }
  return [
    command.intentRef,
    command.actionRef,
    command.outcomeRef,
    ...(command.reasonRef === undefined ? [] : [command.reasonRef]),
    ...(command.receiptRef === undefined ? [] : [command.receiptRef]),
  ];
};

const assertUniqueRefs = (refs: ReadonlyArray<string>): void => {
  if (new Set(refs).size !== refs.length) {
    throw new Error("Device mirror host adjunct repeats a projection reference.");
  }
};

const assertProjectionLaws = (projection: Issue31HostProjection, generatedAtMs: number): void => {
  if (projection.source.observedAtMs > generatedAtMs) {
    throw new Error("Device mirror host adjunct timestamp order is invalid.");
  }
  const refs = [
    projection.source.sourceRef,
    ...(projection.role.grantRef === undefined ? [] : [projection.role.grantRef]),
    ...projection.recordRefs,
    ...projection.permittedActionRefs,
    ...publicRefsForCommand(projection.commandState),
  ];
  if (!refs.every(isIssue31PublicRef)) {
    throw new Error("Device mirror host adjunct contains an unsafe reference.");
  }
  assertUniqueRefs(projection.recordRefs);
  assertUniqueRefs(projection.permittedActionRefs);

  if (projection.gap === "complete" || projection.gap === "partial") {
    if (projection.freshness === "unknown") {
      throw new Error("Device mirror host adjunct projection state is invalid.");
    }
  } else if (
    projection.freshness !== "unknown" ||
    projection.recordRefs.length !== 0 ||
    projection.permittedActionRefs.length !== 0 ||
    projection.commandState.kind !== "idle"
  ) {
    throw new Error("Device mirror host adjunct projection state is invalid.");
  }

  if (projection.role.status === "active" && projection.role.grantRef === undefined) {
    throw new Error("Device mirror host adjunct role state is invalid.");
  }
  if (projection.role.status === "unknown" && projection.role.grantRef !== undefined) {
    throw new Error("Device mirror host adjunct role state is invalid.");
  }
  if (
    projection.role.status !== "active" &&
    (projection.permittedActionRefs.length !== 0 || projection.commandState.kind === "pending")
  ) {
    throw new Error("Device mirror host adjunct role state is invalid.");
  }
  if (
    projection.commandState.kind === "pending" &&
    !projection.permittedActionRefs.includes(projection.commandState.actionRef)
  ) {
    throw new Error("Device mirror host adjunct command state is invalid.");
  }
};

const decodeHostAdjunct = S.decodeUnknownSync(Issue31HostAdjunctSchema);

export const decodeIssue31HostAdjunct = (value: unknown): Issue31HostAdjunct => {
  const adjunct = decodeHostAdjunct(value, { onExcessProperty: "error" });
  assertIssue31AdjunctDeliveryLaw(adjunct);
  if (!isIssue31PublicRef(adjunct.hostRef) || !isIssue31PublicRef(adjunct.snapshotRef)) {
    throw new Error("Device mirror host adjunct contains an unsafe reference.");
  }
  if (adjunct.grantRef !== undefined && !isIssue31PublicRef(adjunct.grantRef)) {
    throw new Error("Device mirror host adjunct contains an unsafe reference.");
  }
  const capabilities = adjunct.projections.map((projection) => projection.capability);
  const expected = [
    "connection_identity",
    "full_auto_runs",
    "provider_accounts",
    "evidence_chain",
  ] as const;
  if (
    new Set(capabilities).size !== capabilities.length ||
    expected.some((value) => !capabilities.includes(value))
  ) {
    throw new Error("Device mirror host adjunct needs four unique capability projections.");
  }
  for (const projection of adjunct.projections) {
    assertProjectionLaws(projection, adjunct.generatedAtMs);
  }
  return adjunct;
};
