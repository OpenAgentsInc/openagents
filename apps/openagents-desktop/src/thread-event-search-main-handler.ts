import {
  DesktopThreadEventSearchChannel,
  decodeDesktopThreadEventSearchRequest,
  decodeDesktopThreadEventSearchResult,
  unavailableDesktopThreadEventSearchResult,
  type DesktopThreadEventSearchRequest,
  type DesktopThreadEventSearchResult,
} from "./thread-event-search-bridge-contract.ts";

export type DesktopThreadEventSearchMainHandler = (
  event: unknown,
  request: unknown,
) => Promise<DesktopThreadEventSearchResult>;

export type DesktopThreadEventSearchMainHandlerDependencies = Readonly<{
  register: (
    channel: typeof DesktopThreadEventSearchChannel,
    handler: DesktopThreadEventSearchMainHandler,
  ) => () => void;
  isTrustedSender: (event: unknown) => boolean;
  search: (request: DesktopThreadEventSearchRequest) => Promise<unknown>;
}>;

/**
 * Main-process registration seam for the fixed accepted-event search channel.
 * Electron owns registration and sender inspection; this boundary owns exact
 * request/result decoding and binds an available projection to its request.
 */
export const registerDesktopThreadEventSearchMainHandler = (
  dependencies: DesktopThreadEventSearchMainHandlerDependencies,
) => {
  let closed = false;
  const handle: DesktopThreadEventSearchMainHandler = async (event, input) => {
    if (closed) return { status: "unavailable", reason: "invalid_request" };
    let trusted = false;
    try {
      trusted = dependencies.isTrustedSender(event);
    } catch {
      return { status: "unavailable", reason: "invalid_request" };
    }
    if (!trusted) return { status: "unavailable", reason: "invalid_request" };

    const request = decodeDesktopThreadEventSearchRequest(input);
    if (request === null) return { status: "unavailable", reason: "invalid_request" };
    try {
      const result = decodeDesktopThreadEventSearchResult(await dependencies.search(request));
      if (
        result === null ||
        (result.status === "available" && result.projection.query !== request.query)
      ) {
        return unavailableDesktopThreadEventSearchResult();
      }
      return result;
    } catch {
      return unavailableDesktopThreadEventSearchResult();
    }
  };

  const unregister = dependencies.register(DesktopThreadEventSearchChannel, handle);
  const close = (): void => {
    if (closed) return;
    closed = true;
    unregister();
  };
  return { close } as const;
};
