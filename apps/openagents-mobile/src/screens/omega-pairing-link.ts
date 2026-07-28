import {
  decodeOmegaBridgePairingText,
  isOmegaPairingLink,
  type OmegaBridgePairingBootstrap,
} from "../workroom/omega-device-bridge-client";

/**
 * The slice of React Native's `Linking` this app consumes. The indirection is
 * not ceremony: `react-native` cannot be imported under the Node test host, so
 * the watcher takes its link source as a value and the screen passes the real
 * one.
 */
export interface OmegaPairingLinkSource {
  readonly getInitialURL: () => Promise<string | null>;
  readonly addEventListener: (
    type: "url",
    listener: (event: { readonly url: string }) => void,
  ) => { readonly remove: () => void };
}

/**
 * Decode an incoming URL as a desktop pairing Universal Link. Returns null for
 * every URL that is not one, because the OS hands this app each link it is
 * registered for and most are not pairing codes.
 */
export const omegaPairingFromUrl = (url: string): OmegaBridgePairingBootstrap | null => {
  if (!isOmegaPairingLink(url)) return null;
  try {
    return decodeOmegaBridgePairingText(url);
  } catch {
    return null;
  }
};

/**
 * Watch for pairing Universal Links: the one the app was opened with (the iOS
 * Camera scan that cold-starts the app) and every one that arrives while it is
 * already running. The returned teardown stops both, including an initial URL
 * that resolves after the screen has unmounted.
 */
export const watchOmegaPairingLinks = (
  source: OmegaPairingLinkSource,
  onPairing: (pairing: OmegaBridgePairingBootstrap) => void,
): (() => void) => {
  let active = true;
  const deliver = (url: string): void => {
    if (!active) return;
    const pairing = omegaPairingFromUrl(url);
    if (pairing !== null) onPairing(pairing);
  };
  void source
    .getInitialURL()
    .then((url) => {
      if (url !== null) deliver(url);
    })
    .catch(() => undefined);
  const subscription = source.addEventListener("url", (event) => deliver(event.url));
  return () => {
    active = false;
    subscription.remove();
  };
};
