import { Effect } from "effect";

import type {
  OmegaBridgePairingBootstrap,
  OmegaDeviceBridgeClient,
  OmegaDeviceBridgeState,
} from "../workroom/omega-device-bridge-client";

const close = (client: OmegaDeviceBridgeClient): void => {
  void Effect.runPromise(client.close()).catch(() => undefined);
};

/**
 * The device mirror grew under an internal issue codename, and its wire
 * tokens (the pairing scope and schema ids) still carry that codename for
 * compatibility with shipped mobile builds. A person must never read it: any
 * message that would surface it — for example a schema decode error quoting a
 * wire literal — collapses to product words instead of leaking internals.
 */
const INTERNAL_REFERENCE = /issue\s*31/i;

export const productSafeNotice = (message: string): string =>
  INTERNAL_REFERENCE.test(message)
    ? "The desktop mirror hit a problem it could not describe. Try pairing again."
    : message;

/**
 * Holds the device bridge for one mount of the home screen.
 *
 * It sits outside the component because the ownership it settles has no other
 * seam: the returned teardown can run while `openBridge` is still pending, and
 * a client that lands after that has to be closed here or its signer keeps the
 * device key alive for the rest of the process (#9264).
 */
export const startOmegaBridgeSession = (
  input: Readonly<{
    /** A caller's bridge outlives this mount, so this session never closes it. */
    bridge: OmegaDeviceBridgeClient | undefined;
    openBridge: () => Promise<OmegaDeviceBridgeClient>;
    pairing: () => Promise<OmegaBridgePairingBootstrap | null>;
    onClient: (client: OmegaDeviceBridgeClient) => void;
    onState: (state: OmegaDeviceBridgeState) => void;
    onNotice: (notice: string) => void;
  }>,
): (() => void) => {
  let active = true;
  let openedClient: OmegaDeviceBridgeClient | null = null;
  let unsubscribe: (() => void) | null = null;

  void (async () => {
    const resolved = input.bridge ?? (await input.openBridge().catch(() => null));
    if (resolved === null) {
      if (active) input.onNotice("The Omega device bridge is unavailable.");
      return;
    }
    if (!active) {
      // Teardown already ran and saw nothing to close, so this arrival is the
      // session's last chance to release the signer holding the device key.
      if (input.bridge === undefined) close(resolved);
      return;
    }
    openedClient = resolved;
    input.onClient(resolved);
    unsubscribe = resolved.subscribe(input.onState);
    const pairing = await input.pairing();
    await Effect.runPromise(
      resolved.connect({ announcements: [], pairing, manualMagicDns: null }).pipe(
        Effect.catch((error) =>
          Effect.sync(() => {
            if (!active) return;
            if (error.reason !== "all_endpoints_failed" || resolved.state().paired) {
              input.onNotice(productSafeNotice(error.message));
            }
          }),
        ),
      ),
    );
  })();

  return () => {
    active = false;
    unsubscribe?.();
    if (input.bridge === undefined && openedClient !== null) close(openedClient);
  };
};
