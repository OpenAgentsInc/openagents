import { Schema as S } from "effect";

import { AggregateType } from "../controller/contracts";

export const AMBIENT_NOTIFICATION_VERSION = "openagents.ambient_notification.v1" as const;
export const AMBIENT_LIVE_ACTIVITY_VERSION = "openagents.live_activity_shell.v1" as const;
export const SHARE_INTAKE_VERSION = "openagents.share_intake.v1" as const;

const OpaqueRef = S.String.pipe(
  S.check(S.isLengthBetween(1, 240), S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:@/-]*$/u)),
);
const Generation = S.Number.pipe(
  S.check(S.isInt(), S.isBetween({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER })),
);
const UnixMilliseconds = S.Number.pipe(
  S.check(S.isInt(), S.isBetween({ minimum: 0, maximum: Number.MAX_SAFE_INTEGER })),
);
const BoundedStatus = S.String.pipe(S.check(S.isLengthBetween(1, 160)));

export const AmbientAttentionState = S.Literals([
  "ready",
  "working",
  "approval",
  "input",
  "failed",
  "done_unseen",
]);
export type AmbientAttentionState = typeof AmbientAttentionState.Type;

export const AmbientNotificationPayload = S.Struct({
  version: S.Literal(AMBIENT_NOTIFICATION_VERSION),
  notificationId: OpaqueRef,
  workspaceId: OpaqueRef,
  aggregateType: AggregateType,
  aggregateId: OpaqueRef,
  attentionState: AmbientAttentionState,
  generation: Generation,
  issuedAt: UnixMilliseconds,
});
export type AmbientNotificationPayload = typeof AmbientNotificationPayload.Type;

export const LiveActivityShellProjection = S.Struct({
  version: S.Literal(AMBIENT_LIVE_ACTIVITY_VERSION),
  workspaceId: OpaqueRef,
  aggregateType: AggregateType,
  aggregateId: OpaqueRef,
  attentionState: AmbientAttentionState,
  status: BoundedStatus,
  generation: Generation,
  updatedAt: UnixMilliseconds,
});
export type LiveActivityShellProjection = typeof LiveActivityShellProjection.Type;

export const ShareIntakeKind = S.Literals(["text", "url", "image"]);
export type ShareIntakeKind = typeof ShareIntakeKind.Type;

export const ShareInboxItem = S.Struct({
  version: S.Literal(SHARE_INTAKE_VERSION),
  intakeId: OpaqueRef,
  kind: ShareIntakeKind,
  value: S.String.pipe(S.check(S.isLengthBetween(1, 8_000))),
  mimeType: S.NullOr(S.String.pipe(S.check(S.isLengthBetween(1, 160)))),
  receivedAt: UnixMilliseconds,
});
export type ShareInboxItem = typeof ShareInboxItem.Type;

const strictDecode =
  <Schema extends S.ConstraintDecoder<unknown, never>>(schema: Schema) =>
  (input: unknown): Schema["Type"] =>
    S.decodeUnknownSync(schema)(input, { onExcessProperty: "error" });

export const decodeAmbientNotification = strictDecode(AmbientNotificationPayload);
export const decodeLiveActivityShell = strictDecode(LiveActivityShellProjection);
export const decodeShareInboxItem = strictDecode(ShareInboxItem);

export const notificationDeepLink = (payload: AmbientNotificationPayload): string => {
  const type = encodeURIComponent(payload.aggregateType);
  const id = encodeURIComponent(payload.aggregateId);
  const workspace = encodeURIComponent(payload.workspaceId);
  return `openagents://work/${type}/${id}?workspaceId=${workspace}`;
};

export const liveActivitySubtitle = (projection: LiveActivityShellProjection): string => {
  const state = projection.attentionState.replaceAll("_", " ");
  return `${state} · ${projection.status}`.slice(0, 160);
};
