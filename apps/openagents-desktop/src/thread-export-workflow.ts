import { Effect } from "effect";

import {
  decodeDesktopThreadExportCreateRequest,
  decodeDesktopThreadExportCreateResult,
  unavailableDesktopThreadExportCreateResult,
  type DesktopThreadExportCreateRequest,
  type DesktopThreadExportCreateResult,
} from "./thread-export-create-bridge-contract.ts";
import {
  decodeDesktopThreadExportWriteResult,
  unavailableDesktopThreadExportWriteResult,
  type DesktopThreadExportWriteRequest,
  type DesktopThreadExportWriteResult,
} from "./thread-export-bridge-contract.ts";

type CreateRejectionReason = Extract<
  DesktopThreadExportCreateResult,
  { status: "rejected" }
>["reason"];
type WriteRejectionReason = Extract<
  DesktopThreadExportWriteResult,
  { status: "rejected" }
>["reason"];

export type DesktopThreadExportWorkflowResult =
  | Readonly<{
      status: "written";
      artifactRef: string;
      artifactSha256: string;
      replaceAuthorized: boolean;
    }>
  | Readonly<{ status: "cancelled" }>
  | Readonly<{ status: "rejected"; stage: "create"; reason: CreateRejectionReason }>
  | Readonly<{ status: "rejected"; stage: "write"; reason: WriteRejectionReason }>;

export type DesktopThreadExportWorkflowDependencies = Readonly<{
  create: (request: DesktopThreadExportCreateRequest) => Promise<unknown>;
  write: (request: DesktopThreadExportWriteRequest) => Promise<unknown>;
}>;

const invokeCreate = (
  dependencies: DesktopThreadExportWorkflowDependencies,
  request: DesktopThreadExportCreateRequest,
): Effect.Effect<DesktopThreadExportCreateResult> =>
  Effect.tryPromise({
    try: () => dependencies.create(request),
    catch: () => undefined,
  }).pipe(
    Effect.map(
      (output) =>
        decodeDesktopThreadExportCreateResult(output, request) ??
        unavailableDesktopThreadExportCreateResult(),
    ),
    Effect.catch(() => Effect.succeed(unavailableDesktopThreadExportCreateResult())),
  );

const invokeWrite = (
  dependencies: DesktopThreadExportWorkflowDependencies,
  request: DesktopThreadExportWriteRequest,
): Effect.Effect<DesktopThreadExportWriteResult> =>
  Effect.tryPromise({
    try: () => dependencies.write(request),
    catch: () => undefined,
  }).pipe(
    Effect.map(
      (output) =>
        decodeDesktopThreadExportWriteResult(output) ?? unavailableDesktopThreadExportWriteResult(),
    ),
    Effect.catch(() => Effect.succeed(unavailableDesktopThreadExportWriteResult())),
  );

/**
 * Renderer-safe orchestration for one owner-only canonical export. The
 * workflow exposes only bounded status and artifact identity; the canonical
 * receipt exists solely long enough to cross the fixed write boundary.
 */
export const runDesktopThreadExportWorkflow = Effect.fn("DesktopThreadExportWorkflow.run")(
  function* (dependencies: DesktopThreadExportWorkflowDependencies, input: unknown) {
    const request = decodeDesktopThreadExportCreateRequest(input);
    if (request === null) {
      return { status: "rejected", stage: "create", reason: "invalid_request" };
    }

    const created = yield* invokeCreate(dependencies, request);
    if (created.status === "rejected") {
      return { status: "rejected", stage: "create", reason: created.reason };
    }

    const written = yield* invokeWrite(dependencies, { receipt: created.receipt });
    if (written.status === "cancelled") return { status: "cancelled" };
    if (written.status === "rejected") {
      return { status: "rejected", stage: "write", reason: written.reason };
    }

    const artifact = created.receipt.result;
    if (
      artifact.status !== "export_created" ||
      written.artifactRef !== artifact.artifactRef ||
      written.artifactSha256 !== artifact.artifactSha256
    ) {
      return { status: "rejected", stage: "write", reason: "transport_unavailable" };
    }
    return written;
  },
);
