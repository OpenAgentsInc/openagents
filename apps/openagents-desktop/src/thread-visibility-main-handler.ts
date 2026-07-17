import {
  DesktopThreadVisibilityApplyChannel,
  decodeDesktopThreadVisibilityApplyRequest,
  decodeDesktopThreadVisibilityApplyResult,
  unavailableDesktopThreadVisibilityApplyResult,
  type DesktopThreadVisibilityApplyRequest,
  type DesktopThreadVisibilityApplyResult,
} from "./thread-visibility-bridge-contract.ts";

export type DesktopThreadVisibilityMainHandler = (
  event: unknown,
  request: unknown,
) => Promise<DesktopThreadVisibilityApplyResult>;

export type DesktopThreadVisibilityMainHandlerDependencies = Readonly<{
  register: (
    channel: typeof DesktopThreadVisibilityApplyChannel,
    handler: DesktopThreadVisibilityMainHandler,
  ) => () => void;
  isTrustedSender: (event: unknown) => boolean;
  makeReceiptRef: () => string;
  observedAt: () => string;
  apply: (input: Readonly<{
    intent: DesktopThreadVisibilityApplyRequest["intent"];
    receiptRef: string;
    observedAt: string;
  }>) => Promise<unknown>;
}>;

/**
 * Main-process registration seam for the fixed visibility channel. Electron
 * owns registration and trusted-sender inspection; this boundary owns exact
 * request/result decoding and keeps receipt metadata host-controlled.
 */
export const registerDesktopThreadVisibilityMainHandler = (
  dependencies: DesktopThreadVisibilityMainHandlerDependencies,
) => {
  let closed = false;
  const handle: DesktopThreadVisibilityMainHandler = async (event, input) => {
    if (closed) return { status: "rejected", reason: "invalid_request" };
    let trusted = false;
    try {
      trusted = dependencies.isTrustedSender(event);
    } catch {
      return { status: "rejected", reason: "invalid_request" };
    }
    if (!trusted) return { status: "rejected", reason: "invalid_request" };

    const request = decodeDesktopThreadVisibilityApplyRequest(input);
    if (request === null) return { status: "rejected", reason: "invalid_request" };
    try {
      const result = await dependencies.apply({
        intent: request.intent,
        receiptRef: dependencies.makeReceiptRef(),
        observedAt: dependencies.observedAt(),
      });
      return (
        decodeDesktopThreadVisibilityApplyResult(result, request) ??
        unavailableDesktopThreadVisibilityApplyResult()
      );
    } catch {
      return unavailableDesktopThreadVisibilityApplyResult();
    }
  };

  const unregister = dependencies.register(DesktopThreadVisibilityApplyChannel, handle);
  const close = (): void => {
    if (closed) return;
    closed = true;
    unregister();
  };
  return { close } as const;
};
