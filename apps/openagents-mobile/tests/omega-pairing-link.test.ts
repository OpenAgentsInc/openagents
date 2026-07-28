import { describe, expect, test } from "vite-plus/test";

import {
  omegaPairingFromUrl,
  watchOmegaPairingLinks,
  type OmegaPairingLinkSource,
} from "../src/screens/omega-pairing-link.ts";
import {
  OMEGA_PAIRING_LINK_BASE,
  type OmegaBridgePairingBootstrap,
} from "../src/workroom/omega-device-bridge-client.ts";

const bootstrap = {
  endpoint: "ws://owner-mac.tail:4317",
  hostPublicKeyHex: "b".repeat(64),
  pairingSecret: "one-time-secret",
  expiresAt: 20_000,
};

const pairingLink = `${OMEGA_PAIRING_LINK_BASE}#${Buffer.from(
  JSON.stringify(bootstrap),
  "utf8",
).toString("base64url")}`;

const settle = (): Promise<void> => new Promise<void>((done) => setImmediate(done));

interface FixtureSource extends OmegaPairingLinkSource {
  readonly emit: (url: string) => void;
  readonly removeCount: () => number;
  readonly resolveInitial: (url: string | null) => void;
}

const fixtureSource = (initial?: string | null): FixtureSource => {
  let resolveInitial: (url: string | null) => void = () => undefined;
  const initialUrl =
    initial === undefined
      ? new Promise<string | null>((resolve) => {
          resolveInitial = resolve;
        })
      : Promise.resolve(initial);
  const listeners = new Set<(event: { readonly url: string }) => void>();
  let removes = 0;
  return {
    getInitialURL: () => initialUrl,
    addEventListener: (_type, listener) => {
      listeners.add(listener);
      return {
        remove: () => {
          removes += 1;
          listeners.delete(listener);
        },
      };
    },
    emit: (url) => {
      for (const listener of listeners) listener({ url });
    },
    removeCount: () => removes,
    resolveInitial: (url) => resolveInitial(url),
  };
};

describe("omegaPairingFromUrl", () => {
  test("decodes a pairing link and refuses everything else without throwing", () => {
    expect(omegaPairingFromUrl(pairingLink)).toEqual(bootstrap);
    expect(omegaPairingFromUrl("https://openagents.com/forum")).toBeNull();
    expect(omegaPairingFromUrl(`${OMEGA_PAIRING_LINK_BASE}#not-json`)).toBeNull();
  });
});

describe("watchOmegaPairingLinks", () => {
  test("delivers the initial URL the app was opened with", async () => {
    const source = fixtureSource(pairingLink);
    const seen: Array<OmegaBridgePairingBootstrap> = [];
    watchOmegaPairingLinks(source, (pairing) => seen.push(pairing));
    await settle();
    expect(seen).toEqual([bootstrap]);
  });

  test("delivers a link that arrives while the app is running", async () => {
    const source = fixtureSource(null);
    const seen: Array<OmegaBridgePairingBootstrap> = [];
    watchOmegaPairingLinks(source, (pairing) => seen.push(pairing));
    await settle();
    source.emit("https://openagents.com/promises");
    source.emit(pairingLink);
    expect(seen).toEqual([bootstrap]);
  });

  test("teardown unsubscribes and fences an initial URL that resolves late", async () => {
    const source = fixtureSource();
    const seen: Array<OmegaBridgePairingBootstrap> = [];
    const stop = watchOmegaPairingLinks(source, (pairing) => seen.push(pairing));
    stop();
    source.resolveInitial(pairingLink);
    await settle();
    expect(seen).toEqual([]);
    expect(source.removeCount()).toBe(1);
  });
});
