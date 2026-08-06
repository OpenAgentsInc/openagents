import { Schema as S } from "effect";
import {
  WorkComposerDraftSchema,
  WorkTranscriptPageSchema,
  type WorkComposerContext,
  type WorkComposerDraft,
  type WorkTranscriptPage,
  type WorkTranscriptRow,
} from "@openagentsinc/all-work-contract/projection";

export {
  WorkComposerDraftSchema,
  WorkTranscriptPageSchema,
  type WorkComposerContext,
  type WorkComposerDraft,
  type WorkTranscriptPage,
  type WorkTranscriptRow,
};

export const MOBILE_CONTROLLER_VERSION = "openagents.mobile_controller.v1" as const;

export const AggregateType = S.Literals(["thread", "issue", "order"]);
export type AggregateType = typeof AggregateType.Type;

export class ControllerActor extends S.Class<ControllerActor>("ControllerActor")({
  userId: S.String,
  name: S.String,
  avatarUrl: S.String,
}) {}

export class ControllerWorkspace extends S.Class<ControllerWorkspace>("ControllerWorkspace")({
  workspaceId: S.String,
  label: S.String,
}) {}

export class ControllerRotatedTokens extends S.Class<ControllerRotatedTokens>(
  "ControllerRotatedTokens",
)({
  access: S.String,
  refresh: S.String,
  expiresIn: S.Number,
}) {}

export class ControllerBootstrap extends S.Class<ControllerBootstrap>("ControllerBootstrap")({
  version: S.Literal(MOBILE_CONTROLLER_VERSION),
  token: S.String,
  convexUrl: S.String,
  actor: ControllerActor,
  workspace: ControllerWorkspace,
  rotatedTokens: S.optional(ControllerRotatedTokens),
}) {}

export class PendingRequest extends S.Class<PendingRequest>("PendingRequest")({
  requestId: S.String,
  kind: S.Literals(["approval", "input"]),
  summary: S.String,
  recordedAt: S.Number,
}) {}

export class AttentionShell extends S.Class<AttentionShell>("AttentionShell")({
  aggregateType: AggregateType,
  aggregateId: S.String,
  label: S.String,
  identifier: S.NullOr(S.String),
  status: S.String,
  attentionState: S.String,
  attentionRank: S.Number,
  pendingApprovalCount: S.Number,
  pendingInputCount: S.Number,
  visibility: S.String,
  generation: S.Number,
  createdAt: S.Number,
  updatedAt: S.Number,
  pendingRequests: S.Array(PendingRequest),
}) {}

export const AttentionInbox = S.Array(AttentionShell);

export class WorkShell extends S.Class<WorkShell>("WorkShell")({
  aggregateType: AggregateType,
  aggregateId: S.String,
  status: S.String,
  attentionState: S.String,
  pendingApprovalCount: S.Number,
  pendingInputCount: S.Number,
  generation: S.Number,
  updatedAt: S.Number,
}) {}

export class ControllerTransportReceipt extends S.Class<ControllerTransportReceipt>(
  "ControllerTransportReceipt",
)({
  status: S.Literals(["accepted", "duplicate", "rejected"]),
  receiptRef: S.String,
  code: S.optional(S.String),
  detail: S.optional(S.String),
  rotatedTokens: S.optional(ControllerRotatedTokens),
}) {}

export type ControllerTarget = Readonly<{
  aggregateType: AggregateType;
  aggregateId: string;
  expectedGeneration?: number;
}>;

export const decodeControllerBootstrap = S.decodeUnknownSync(ControllerBootstrap);
export const decodeAttentionInbox = S.decodeUnknownSync(AttentionInbox);
export const decodeWorkTranscriptPage = (input: unknown): WorkTranscriptPage =>
  S.decodeUnknownSync(WorkTranscriptPageSchema)(input, { onExcessProperty: "error" });
export const decodeWorkComposerDraft = (input: unknown): WorkComposerDraft =>
  S.decodeUnknownSync(WorkComposerDraftSchema)(input, { onExcessProperty: "error" });
export const decodeWorkShell = S.decodeUnknownSync(S.NullOr(WorkShell));
export const decodeTransportReceipt = S.decodeUnknownSync(ControllerTransportReceipt);
