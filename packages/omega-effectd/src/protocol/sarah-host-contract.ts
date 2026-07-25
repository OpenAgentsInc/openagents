import { Schema } from "effect";

const ISSUE31_PUBLIC_REF_PATTERN =
  /^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/;
const PublicRef = Schema.String.pipe(
  Schema.check(Schema.isLengthBetween(3, 256), Schema.isPattern(ISSUE31_PUBLIC_REF_PATTERN)),
);
const Cursor = Schema.String.pipe(Schema.check(Schema.isLengthBetween(1, 256)));
const PageLimit = Schema.Number.pipe(
  Schema.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 64 })),
);
const Generation = Schema.Number.pipe(
  Schema.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER })),
);
const UnixSeconds = Schema.Number.pipe(
  Schema.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER })),
);
const MessageText = Schema.String.pipe(Schema.check(Schema.isLengthBetween(1, 8_000)));
const PairingScope = Schema.Literals([
  "observe_issue31",
  "send_message",
  "interrupt_turn",
  "control_full_auto",
  "request_provider_handoff",
  "act_in_community",
]);

export const SarahSessionStatusParams = Schema.Struct({});
export interface SarahSessionStatusParams extends Schema.Schema.Type<
  typeof SarahSessionStatusParams
> {}

export const SarahBootstrapParams = Schema.Struct({});
export interface SarahBootstrapParams extends Schema.Schema.Type<typeof SarahBootstrapParams> {}

export const SarahRoomSnapshotParams = Schema.Struct({
  cursor: Schema.optionalKey(Cursor),
  limit: Schema.optionalKey(PageLimit),
  transcriptCursor: Schema.optionalKey(Cursor),
  activityCursor: Schema.optionalKey(Cursor),
  transcriptLimit: Schema.optionalKey(PageLimit),
  activityLimit: Schema.optionalKey(PageLimit),
});
export interface SarahRoomSnapshotParams extends Schema.Schema.Type<
  typeof SarahRoomSnapshotParams
> {}

export const SarahSendMessageParams = Schema.Struct({
  text: MessageText,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
});
export interface SarahSendMessageParams extends Schema.Schema.Type<typeof SarahSendMessageParams> {}

export const SarahInterruptTurnParams = Schema.Struct({
  turnRef: PublicRef,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
});
export interface SarahInterruptTurnParams extends Schema.Schema.Type<
  typeof SarahInterruptTurnParams
> {}

export const SarahRenewDeviceGrantParams = Schema.Struct({
  grantRef: PublicRef,
  scopes: Schema.Array(PairingScope).pipe(
    Schema.check(Schema.isMinLength(1), Schema.isMaxLength(6)),
  ),
  expiresAt: UnixSeconds,
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
});
export interface SarahRenewDeviceGrantParams extends Schema.Schema.Type<
  typeof SarahRenewDeviceGrantParams
> {}

export const SarahRevokeDeviceGrantParams = Schema.Struct({
  grantRef: PublicRef,
  reasonRef: Schema.optionalKey(PublicRef),
  idempotencyRef: PublicRef,
  expectedGeneration: Generation,
});
export interface SarahRevokeDeviceGrantParams extends Schema.Schema.Type<
  typeof SarahRevokeDeviceGrantParams
> {}

export type OmegaEffectdSarahHostMethod =
  | "sarah_session_status"
  | "sarah_bootstrap"
  | "sarah_room_snapshot"
  | "sarah_send_message"
  | "sarah_interrupt_turn"
  | "sarah_renew_device_grant"
  | "sarah_revoke_device_grant";

export type OmegaEffectdSarahHostParams = Readonly<{
  sarah_session_status: SarahSessionStatusParams;
  sarah_bootstrap: SarahBootstrapParams;
  sarah_room_snapshot: SarahRoomSnapshotParams;
  sarah_send_message: SarahSendMessageParams;
  sarah_interrupt_turn: SarahInterruptTurnParams;
  sarah_renew_device_grant: SarahRenewDeviceGrantParams;
  sarah_revoke_device_grant: SarahRevokeDeviceGrantParams;
}>;

const decodeOptions = { onExcessProperty: "error" } as const;

const decodeSarahSessionStatusParamsSchema = Schema.decodeUnknownSync(
  SarahSessionStatusParams,
  decodeOptions,
);
const decodeSarahBootstrapParamsSchema = Schema.decodeUnknownSync(
  SarahBootstrapParams,
  decodeOptions,
);
const decodeSarahRoomSnapshotParamsSchema = Schema.decodeUnknownSync(
  SarahRoomSnapshotParams,
  decodeOptions,
);
const decodeSarahSendMessageParamsSchema = Schema.decodeUnknownSync(
  SarahSendMessageParams,
  decodeOptions,
);
const decodeSarahInterruptTurnParamsSchema = Schema.decodeUnknownSync(
  SarahInterruptTurnParams,
  decodeOptions,
);
const decodeSarahRenewDeviceGrantParamsSchema = Schema.decodeUnknownSync(
  SarahRenewDeviceGrantParams,
  decodeOptions,
);
const decodeSarahRevokeDeviceGrantParamsSchema = Schema.decodeUnknownSync(
  SarahRevokeDeviceGrantParams,
  decodeOptions,
);

const hasExactKeys = (value: unknown, permittedKeys: ReadonlySet<string>): boolean =>
  value !== null &&
  typeof value === "object" &&
  !Array.isArray(value) &&
  Object.keys(value).every((key) => permittedKeys.has(key));

const decodeExact = <Params>(
  input: unknown,
  permittedKeys: ReadonlySet<string>,
  decode: (value: unknown) => Params,
): Params => {
  if (!hasExactKeys(input, permittedKeys)) {
    throw new Error("The Sarah host request has an unknown parameter.");
  }
  return decode(input);
};

const noKeys = new Set<string>();
const roomSnapshotKeys = new Set([
  "cursor",
  "limit",
  "transcriptCursor",
  "activityCursor",
  "transcriptLimit",
  "activityLimit",
]);
const sendMessageKeys = new Set(["text", "idempotencyRef", "expectedGeneration"]);
const interruptTurnKeys = new Set(["turnRef", "idempotencyRef", "expectedGeneration"]);
const renewDeviceGrantKeys = new Set([
  "grantRef",
  "scopes",
  "expiresAt",
  "idempotencyRef",
  "expectedGeneration",
]);
const revokeDeviceGrantKeys = new Set([
  "grantRef",
  "reasonRef",
  "idempotencyRef",
  "expectedGeneration",
]);

export const decodeSarahSessionStatusParams = (input: unknown): SarahSessionStatusParams =>
  decodeExact(input, noKeys, decodeSarahSessionStatusParamsSchema);
export const decodeSarahBootstrapParams = (input: unknown): SarahBootstrapParams =>
  decodeExact(input, noKeys, decodeSarahBootstrapParamsSchema);
export const decodeSarahRoomSnapshotParams = (input: unknown): SarahRoomSnapshotParams =>
  decodeExact(input, roomSnapshotKeys, decodeSarahRoomSnapshotParamsSchema);
export const decodeSarahSendMessageParams = (input: unknown): SarahSendMessageParams =>
  decodeExact(input, sendMessageKeys, decodeSarahSendMessageParamsSchema);
export const decodeSarahInterruptTurnParams = (input: unknown): SarahInterruptTurnParams =>
  decodeExact(input, interruptTurnKeys, decodeSarahInterruptTurnParamsSchema);
export const decodeSarahRenewDeviceGrantParams = (input: unknown): SarahRenewDeviceGrantParams =>
  decodeExact(input, renewDeviceGrantKeys, decodeSarahRenewDeviceGrantParamsSchema);
export const decodeSarahRevokeDeviceGrantParams = (input: unknown): SarahRevokeDeviceGrantParams =>
  decodeExact(input, revokeDeviceGrantKeys, decodeSarahRevokeDeviceGrantParamsSchema);

const isIssue31PublicRef = (value: string): boolean => {
  return value.length >= 3 && value.length <= 256 && ISSUE31_PUBLIC_REF_PATTERN.test(value);
};

export const hasBoundedUtf8Fields = (
  params:
    | SarahRoomSnapshotParams
    | SarahSendMessageParams
    | SarahInterruptTurnParams
    | SarahRenewDeviceGrantParams
    | SarahRevokeDeviceGrantParams,
): boolean => {
  for (const [field, value] of Object.entries(params)) {
    if (typeof value !== "string") continue;
    const maximum = field === "text" ? 8_000 : 256;
    if (Buffer.byteLength(value, "utf8") > maximum) return false;
    if (
      (field === "idempotencyRef" ||
        field === "turnRef" ||
        field === "grantRef" ||
        field === "reasonRef") &&
      !isIssue31PublicRef(value)
    ) {
      return false;
    }
    if (field === "text" && value.trim().length === 0) return false;
  }
  return true;
};

export const hasUniqueGrantScopes = (params: SarahRenewDeviceGrantParams): boolean =>
  new Set(params.scopes).size === params.scopes.length;
