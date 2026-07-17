import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

export const DesktopThreadExportWriteChannel = "openagents:thread-export:write" as const;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Sha256 = S.String.check(S.isPattern(/^[a-f0-9]{64}$/));

export type DesktopThreadExportWriteRequest = Readonly<{
  receipt: ThreadDisclosureReceipt;
}>;

export const DesktopThreadExportWriteResult = S.Union([
  S.Struct({ status: S.Literal("cancelled") }),
  S.Struct({
    status: S.Literal("written"),
    artifactRef: Ref,
    artifactSha256: Sha256,
    replaceAuthorized: S.Boolean,
  }),
  S.Struct({
    status: S.Literal("rejected"),
    reason: S.Literals([
      "invalid_request",
      "invalid_receipt",
      "unsupported_export",
      "artifact_missing",
      "artifact_corrupt",
      "destination_unavailable",
      "destination_invalid",
      "destination_exists",
      "write_failed",
      "transport_unavailable",
    ]),
  }),
]);
export type DesktopThreadExportWriteResult = typeof DesktopThreadExportWriteResult.Type;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

export const decodeDesktopThreadExportWriteRequest = (
  input: unknown,
): DesktopThreadExportWriteRequest | null => {
  if (
    typeof input !== "object" ||
    input === null ||
    Array.isArray(input) ||
    !ownKeysAre(input, ["receipt"])
  )
    return null;
  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(Reflect.get(input, "receipt"));
  } catch {
    return null;
  }
  if (
    receipt.kind !== "thread.export.create" ||
    receipt.result.status !== "export_created" ||
    receipt.result.format !== "canonical_event_bundle" ||
    receipt.result.artifactAudience.kind !== "owner_only"
  ) {
    return null;
  }
  return { receipt };
};

const decodeResult = S.decodeUnknownSync(DesktopThreadExportWriteResult);

export const decodeDesktopThreadExportWriteResult = (
  input: unknown,
): DesktopThreadExportWriteResult | null => {
  const status =
    typeof input === "object" && input !== null ? Reflect.get(input, "status") : undefined;
  const exact =
    status === "cancelled"
      ? ownKeysAre(input, ["status"])
      : status === "written"
        ? ownKeysAre(input, ["status", "artifactRef", "artifactSha256", "replaceAuthorized"])
        : status === "rejected"
          ? ownKeysAre(input, ["status", "reason"])
          : false;
  if (!exact) return null;
  try {
    return decodeResult(input);
  } catch {
    return null;
  }
};

export const unavailableDesktopThreadExportWriteResult = (): DesktopThreadExportWriteResult => ({
  status: "rejected",
  reason: "transport_unavailable",
});

export type DesktopThreadExportWriteInvoker = (
  channel: typeof DesktopThreadExportWriteChannel,
  request: DesktopThreadExportWriteRequest,
) => Promise<unknown>;

/**
 * Sandboxed renderer-to-main invocation boundary. Only the fixed channel and
 * exact ref-only receipt cross preload; native errors and malformed replies do
 * not.
 */
export const invokeDesktopThreadExportWrite = async (
  invoke: DesktopThreadExportWriteInvoker,
  input: unknown,
): Promise<DesktopThreadExportWriteResult> => {
  const request = decodeDesktopThreadExportWriteRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" };
  try {
    return (
      decodeDesktopThreadExportWriteResult(
        await invoke(DesktopThreadExportWriteChannel, request),
      ) ?? unavailableDesktopThreadExportWriteResult()
    );
  } catch {
    return unavailableDesktopThreadExportWriteResult();
  }
};
