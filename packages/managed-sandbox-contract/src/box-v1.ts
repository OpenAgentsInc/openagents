import { Schema as S } from "effect";

import { BOX_V1_TRANSLATOR_REF } from "./provenance.ts";
import {
  BoxProjectionCursorSchema,
  NonNegativeInt,
  SandboxRef,
  type ManagedSandboxResource,
} from "./schemas.ts";

export type BoxV1Operation = Readonly<{
  sdkMethod: string;
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  path: string;
  successStatuses: ReadonlyArray<number>;
  errorCodes: ReadonlyArray<string>;
}>;

const errors = [
  "authentication_required",
  "permission_denied",
  "resource_not_found",
  "conflict",
  "validation_failed",
  "rate_limited",
  "capacity_unavailable",
  "upstream_unavailable",
] as const;

/** Exact Phase-1 method/status/error corpus admitted by SBX-00. */
export const BOX_V1_PHASE1_OPERATIONS = [
  { sdkMethod: "me", method: "GET", path: "/v1/me", successStatuses: [200], errorCodes: errors },
  {
    sdkMethod: "limits",
    method: "GET",
    path: "/v1/limits",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "boxes",
    method: "GET",
    path: "/v1/boxes",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "create",
    method: "POST",
    path: "/v1/boxes",
    successStatuses: [200, 201, 202],
    errorCodes: errors,
  },
  {
    sdkMethod: "get",
    method: "GET",
    path: "/v1/boxes/{id}",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "update",
    method: "PATCH",
    path: "/v1/boxes/{id}",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "remove",
    method: "DELETE",
    path: "/v1/boxes/{id}",
    successStatuses: [200, 202, 204],
    errorCodes: errors,
  },
  {
    sdkMethod: "stop",
    method: "POST",
    path: "/v1/boxes/{id}/stop",
    successStatuses: [200, 202],
    errorCodes: errors,
  },
  {
    sdkMethod: "resume",
    method: "POST",
    path: "/v1/boxes/{id}/resume",
    successStatuses: [200, 202],
    errorCodes: errors,
  },
  {
    sdkMethod: "prompt",
    method: "POST",
    path: "/v1/boxes/{id}/prompt",
    successStatuses: [200, 202],
    errorCodes: errors,
  },
  {
    sdkMethod: "promptRunStatus",
    method: "GET",
    path: "/v1/boxes/{id}/prompts/{promptId}",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "events",
    method: "GET",
    path: "/v1/boxes/{id}/events",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "interrupt",
    method: "POST",
    path: "/v1/boxes/{id}/interrupt",
    successStatuses: [200, 202],
    errorCodes: errors,
  },
  {
    sdkMethod: "readFile",
    method: "GET",
    path: "/v1/boxes/{id}/files",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "writeFile",
    method: "PUT",
    path: "/v1/boxes/{id}/files",
    successStatuses: [200, 201],
    errorCodes: errors,
  },
  {
    sdkMethod: "command",
    method: "POST",
    path: "/v1/boxes/{id}/commands",
    successStatuses: [200],
    errorCodes: errors,
  },
  {
    sdkMethod: "artifact",
    method: "GET",
    path: "/v1/boxes/{id}/artifacts?path={path}",
    successStatuses: [200],
    errorCodes: errors,
  },
] as const satisfies ReadonlyArray<BoxV1Operation>;

export const BOX_V1_UNSUPPORTED_SDK_METHODS = [
  "apiKeys",
  "desktop",
  "fork",
  "getLatestBoxSnapshot",
  "getSnapshotDownload",
  "getSnapshotFile",
  "getSnapshotTree",
  "listBoxSnapshots",
  "listSnapshots",
  "repos",
  "secrets",
  "selectRepo",
  "sshKey",
  "updateSecrets",
] as const;

export const BOX_CAPABILITY_NOT_IMPLEMENTED_STATUS = 501 as const;
export const BOX_CAPABILITY_NOT_IMPLEMENTED_CODE = "capability_not_implemented" as const;

export const BoxV1ErrorEnvelopeSchema = S.Struct({
  ok: S.Literal(false),
  type: S.Literal("error"),
  status: S.Number,
  code: SandboxRef,
  message: S.String,
  requestId: S.optionalKey(SandboxRef),
  error: S.Struct({
    code: SandboxRef,
    message: S.String,
    status: S.Number,
    details: S.optionalKey(S.Unknown),
  }),
});
export type BoxV1ErrorEnvelope = typeof BoxV1ErrorEnvelopeSchema.Type;

export const capabilityNotImplemented = (sdkMethod: string): BoxV1ErrorEnvelope => ({
  ok: false,
  type: "error",
  status: BOX_CAPABILITY_NOT_IMPLEMENTED_STATUS,
  code: BOX_CAPABILITY_NOT_IMPLEMENTED_CODE,
  message: `Box SDK method ${sdkMethod} is outside the OpenAgents Box-v1 compatibility profile`,
  error: {
    code: BOX_CAPABILITY_NOT_IMPLEMENTED_CODE,
    message: `Box SDK method ${sdkMethod} is not implemented`,
    status: BOX_CAPABILITY_NOT_IMPLEMENTED_STATUS,
    details: { sdkMethod, translatorRef: BOX_V1_TRANSLATOR_REF },
  },
});

export const BoxV1BoxStateSchema = S.Literals([
  "init",
  "provisioning",
  "provisioned",
  "cloning",
  "ready",
  "idle",
  "running",
  "archiving",
  "archived",
  "error",
]);
export type BoxV1BoxState = typeof BoxV1BoxStateSchema.Type;

export const BoxV1BoxSchema = S.Struct({
  id: SandboxRef,
  name: S.String,
  state: BoxV1BoxStateSchema,
  url: S.NullOr(S.String),
  ip: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archiveAfter: S.NullOr(S.String),
  desktopAvailable: S.Boolean,
  desktopUrl: S.NullOr(S.String),
  snapshotAvailable: S.Boolean,
  snapshotCompletedAt: S.NullOr(S.String),
  subdomain: S.NullOr(S.String),
  lastSnapshotAttemptAt: S.NullOr(S.String),
  lastSnapshotStatus: S.NullOr(
    S.Literals(["queued", "in_progress", "completed", "failed", "cancelled"]),
  ),
  openagents: S.Struct({
    translatorRef: S.Literal(BOX_V1_TRANSLATOR_REF),
    resourceGeneration: NonNegativeInt,
    version: NonNegativeInt,
    leaseState: S.String,
    guestState: S.String,
    filesystemState: S.String,
    runtimeState: S.String,
    acceptingWork: S.Boolean,
    cleanupComplete: S.Boolean,
  }),
});
export type BoxV1Box = typeof BoxV1BoxSchema.Type;

const BoxV1PageInfoSchema = S.Struct({
  nextCursor: S.NullOr(SandboxRef),
  hasMore: S.Boolean,
  limit: S.Number,
});

export const BoxV1MeResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("user.info"),
  user: S.Struct({ login: S.String, email: S.NullOr(S.String) }),
});

export const BoxV1LimitsResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("limits.info"),
  accessTier: S.Literal("openagents_managed"),
  canStart: S.Boolean,
  activeBoxes: NonNegativeInt,
  activeStates: S.Array(S.String),
  maxActiveBoxes: NonNegativeInt,
  billingStatus: S.Literal("openagents_receipt_first"),
});

export const BoxV1BoxInfoResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("box.info"),
  box: BoxV1BoxSchema,
});

export const BoxV1BoxListResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("boxes.list"),
  boxes: S.Array(BoxV1BoxSchema),
  pageInfo: BoxV1PageInfoSchema,
});

export const BoxV1CreateResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("box.created"),
  status: S.Literal("provisioning"),
  ttlSeconds: S.Number,
  box: BoxV1BoxSchema,
});

export const BoxV1ActionResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.String,
  id: SandboxRef,
  status: S.String,
  box: BoxV1BoxSchema,
});

export const BoxV1DeleteResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("box.deleted"),
  id: SandboxRef,
  status: S.Literals(["deleting", "deleted"]),
});

export const BoxV1PromptRunSchema = S.Struct({
  id: SandboxRef,
  promptId: SandboxRef,
  boxId: SandboxRef,
  status: S.Literals(["sending", "queued", "running", "finished", "failed"]),
  done: S.Boolean,
  createdAt: S.String,
  model: S.NullOr(S.String),
  reasoningEffort: S.NullOr(S.String),
});

export const BoxV1PromptResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("prompt.queued"),
  id: SandboxRef,
  promptId: SandboxRef,
  promptRun: BoxV1PromptRunSchema,
  status: S.Literal("queued"),
  provider: S.String,
  model: S.NullOr(S.String),
  reasoningEffort: S.NullOr(S.String),
});

export const BoxV1PromptRunResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("prompt.run"),
  id: SandboxRef,
  promptRun: BoxV1PromptRunSchema,
});

export const BoxV1FileReadResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("file.read"),
  success: S.Literal(true),
  path: S.String,
  encoding: S.Literals(["utf8", "base64"]),
  size: NonNegativeInt,
  content: S.String,
});

export const BoxV1FileWriteResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("file.written"),
  success: S.Literal(true),
  path: S.String,
  encoding: S.Literals(["utf8", "base64"]),
  size: NonNegativeInt,
});

export const BoxV1CommandResponseSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("command.finished"),
  success: S.Boolean,
  exitCode: S.NullOr(S.Number),
  signal: S.NullOr(S.String),
  stdout: S.String,
  stderr: S.String,
  stdoutTruncated: S.Boolean,
  stderrTruncated: S.Boolean,
  timedOut: S.Boolean,
  cwd: S.String,
  startedAt: S.String,
  finishedAt: S.String,
});

const boxStateForLifecycle = (
  lifecycle: ManagedSandboxResource["facts"]["lifecycle"],
): BoxV1BoxState => {
  switch (lifecycle) {
    case "ready":
    case "idle":
    case "running":
      return lifecycle;
    case "stopping":
    case "deleting":
      return "archiving";
    case "stopped":
    case "deleted":
      return "archived";
    case "failed":
    case "recovery_required":
      return "error";
    case "provisioning":
    case "resuming":
      return "provisioning";
  }
};

/** Lossy, public-safe Box projection. Provider topology and private ingress stay absent. */
export const projectManagedSandboxToBoxV1 = (resource: ManagedSandboxResource): BoxV1Box =>
  S.decodeUnknownSync(BoxV1BoxSchema)({
    id: resource.sandboxRef,
    name: resource.sandboxRef,
    state: boxStateForLifecycle(resource.facts.lifecycle),
    url: null,
    ip: null,
    createdAt: resource.createdAt,
    updatedAt: resource.updatedAt,
    archiveAfter: resource.lease.expiresAt,
    desktopAvailable: false,
    desktopUrl: null,
    snapshotAvailable: false,
    snapshotCompletedAt: null,
    subdomain: null,
    lastSnapshotAttemptAt: null,
    lastSnapshotStatus: null,
    openagents: {
      translatorRef: BOX_V1_TRANSLATOR_REF,
      resourceGeneration: resource.resourceGeneration,
      version: resource.version,
      leaseState: resource.facts.leaseState,
      guestState: resource.facts.guestState,
      filesystemState: resource.facts.filesystemState,
      runtimeState: resource.facts.runtimeState,
      acceptingWork: resource.facts.acceptingWork,
      cleanupComplete: resource.facts.cleanupComplete,
    },
  });

export const BoxV1ProjectedEventPageSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("events.list"),
  id: SandboxRef,
  events: S.Array(
    S.Struct({
      id: SandboxRef,
      type: SandboxRef,
      timestamp: S.Number,
      taskId: S.optionalKey(S.NullOr(SandboxRef)),
      data: S.optionalKey(S.Unknown),
    }),
  ),
  pageInfo: BoxV1PageInfoSchema,
  projection: BoxProjectionCursorSchema,
});
export type BoxV1ProjectedEventPage = typeof BoxV1ProjectedEventPageSchema.Type;
