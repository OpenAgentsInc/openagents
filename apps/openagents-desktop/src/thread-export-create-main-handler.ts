import {
  DesktopThreadExportCreateChannel,
  decodeDesktopThreadExportCreateRequest,
  decodeDesktopThreadExportCreateResult,
  unavailableDesktopThreadExportCreateResult,
  type DesktopThreadExportCreateRequest,
  type DesktopThreadExportCreateResult,
} from "./thread-export-create-bridge-contract.ts";

export type DesktopThreadExportCreateMainHandler = (
  event: unknown,
  request: unknown,
) => Promise<DesktopThreadExportCreateResult>;

export type DesktopThreadExportCreateMainHandlerDependencies = Readonly<{
  register: (
    channel: typeof DesktopThreadExportCreateChannel,
    handler: DesktopThreadExportCreateMainHandler,
  ) => () => void;
  isTrustedSender: (event: unknown) => boolean;
  execute: (intent: DesktopThreadExportCreateRequest["intent"]) => Promise<unknown>;
}>;

/**
 * Main-process registration seam for the fixed export-creation channel. The
 * Electron adapter owns registration/removal; this boundary owns sender and
 * payload validation plus identity-bound result decoding.
 */
export const registerDesktopThreadExportCreateMainHandler = (
  dependencies: DesktopThreadExportCreateMainHandlerDependencies,
) => {
  let closed = false;
  const handle: DesktopThreadExportCreateMainHandler = async (event, input) => {
    if (closed) return { status: "rejected", reason: "invalid_request" };
    let trusted = false;
    try {
      trusted = dependencies.isTrustedSender(event);
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }
    if (!trusted) return { status: "rejected", reason: "invalid_request" };

    const request = decodeDesktopThreadExportCreateRequest(input);
    if (request === null) return { status: "rejected", reason: "invalid_request" };
    try {
      return (
        decodeDesktopThreadExportCreateResult(
          await dependencies.execute(request.intent),
          request,
        ) ?? unavailableDesktopThreadExportCreateResult()
      );
    } catch {
      return unavailableDesktopThreadExportCreateResult();
    }
  };

  const unregister = dependencies.register(DesktopThreadExportCreateChannel, handle);
  const close = (): void => {
    if (closed) return;
    closed = true;
    unregister();
  };
  return { close } as const;
};
