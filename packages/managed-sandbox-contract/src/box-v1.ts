import { Schema as S } from "effect";

import { BOX_V1_TRANSLATOR_REF } from "./provenance.ts";
import { BoxProjectionCursorSchema, SandboxRef } from "./schemas.ts";

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

export const BoxV1ProjectedEventPageSchema = S.Struct({
  ok: S.Literal(true),
  type: S.Literal("events"),
  id: SandboxRef,
  events: S.Array(
    S.Struct({
      id: SandboxRef,
      type: SandboxRef,
      timestamp: S.String,
      taskId: S.optionalKey(SandboxRef),
      data: S.optionalKey(S.Unknown),
    }),
  ),
  projection: BoxProjectionCursorSchema,
});
export type BoxV1ProjectedEventPage = typeof BoxV1ProjectedEventPageSchema.Type;
