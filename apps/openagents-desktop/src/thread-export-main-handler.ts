import {
  DesktopThreadExportWriteChannel,
  decodeDesktopThreadExportWriteRequest,
  decodeDesktopThreadExportWriteResult,
  unavailableDesktopThreadExportWriteResult,
  type DesktopThreadExportWriteRequest,
  type DesktopThreadExportWriteResult,
} from "./thread-export-bridge-contract.ts";

export type DesktopThreadExportMainHandler = (
  event: unknown,
  request: unknown,
) => Promise<DesktopThreadExportWriteResult>;

export type DesktopThreadExportMainHandlerDependencies = Readonly<{
  register: (
    channel: typeof DesktopThreadExportWriteChannel,
    handler: DesktopThreadExportMainHandler,
  ) => () => void;
  isTrustedSender: (event: unknown) => boolean;
  write: (receipt: DesktopThreadExportWriteRequest["receipt"]) => Promise<unknown>;
}>;

/**
 * Main-process registration seam for the fixed canonical-export channel. The
 * Electron adapter owns registration/removal; this boundary owns sender and
 * payload validation plus path-free result decoding.
 */
export const registerDesktopThreadExportMainHandler = (
  dependencies: DesktopThreadExportMainHandlerDependencies,
) => {
  let closed = false;
  const handle: DesktopThreadExportMainHandler = async (event, input) => {
    if (closed) return { status: "rejected", reason: "invalid_request" };
    let trusted = false;
    try {
      trusted = dependencies.isTrustedSender(event);
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }
    if (!trusted) return { status: "rejected", reason: "invalid_request" };

    const request = decodeDesktopThreadExportWriteRequest(input);
    if (request === null) return { status: "rejected", reason: "invalid_request" };
    try {
      return (
        decodeDesktopThreadExportWriteResult(await dependencies.write(request.receipt)) ??
        unavailableDesktopThreadExportWriteResult()
      );
    } catch {
      return unavailableDesktopThreadExportWriteResult();
    }
  };

  const unregister = dependencies.register(DesktopThreadExportWriteChannel, handle);
  const close = (): void => {
    if (closed) return;
    closed = true;
    unregister();
  };
  return { close } as const;
};
