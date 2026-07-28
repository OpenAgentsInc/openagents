import { describe, expect, test } from "vite-plus/test";

import {
  OMEGA_DESKTOP_PAIRING_BOOTSTRAP_SCHEMA,
  OMEGA_DEVICE_BRIDGE_PROTOCOL,
  OMEGA_PAIRING_LINK_BASE,
  decodeOmegaBridgePairingText,
  isOmegaPairingLink,
} from "../src/workroom/omega-device-bridge-client.ts";

const hostPublicKeyHex = "b".repeat(64);
const pairingSecret = "c".repeat(64);

/** The exact JSON the desktop's `PairingBootstrap::qr` serializes today. */
const desktopPayload = {
  schema: OMEGA_DESKTOP_PAIRING_BOOTSTRAP_SCHEMA,
  magicDnsName: "macbook-pro-m5.tailaeab8f.ts.net",
  port: 4_317,
  protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
  hostPublicKeyHex,
  pairingSecret,
  generation: 3,
  issuedAt: 1_000,
  expiresAt: 301_000,
};

/** The client's own bootstrap shape, already carrying a dialable endpoint. */
const endpointPayload = {
  endpoint: "wss://owner-mac.tail:4317",
  hostPublicKeyHex,
  pairingSecret: "one-time-secret",
  expiresAt: 20_000,
};

const decodedDesktopPayload = {
  endpoint: "ws://macbook-pro-m5.tailaeab8f.ts.net:4317",
  hostPublicKeyHex,
  pairingSecret,
  expiresAt: 301_000,
};

const asPairingLink = (payload: unknown): string =>
  `${OMEGA_PAIRING_LINK_BASE}#${Buffer.from(JSON.stringify(payload), "utf8").toString("base64url")}`;

describe("decodeOmegaBridgePairingText", () => {
  test("accepts the desktop QR's raw device_pairing.v1 JSON and derives the endpoint", () => {
    expect(decodeOmegaBridgePairingText(JSON.stringify(desktopPayload))).toEqual(
      decodedDesktopPayload,
    );
  });

  test("accepts the client bootstrap JSON unchanged", () => {
    expect(decodeOmegaBridgePairingText(JSON.stringify(endpointPayload))).toEqual(endpointPayload);
  });

  test("accepts the Universal Link wrapping the desktop payload", () => {
    expect(decodeOmegaBridgePairingText(asPairingLink(desktopPayload))).toEqual(
      decodedDesktopPayload,
    );
  });

  test("accepts the Universal Link wrapping the endpoint payload", () => {
    expect(decodeOmegaBridgePairingText(asPairingLink(endpointPayload))).toEqual(endpointPayload);
  });

  test("accepts a trailing slash before the fragment and surrounding whitespace", () => {
    const link = asPairingLink(desktopPayload).replace("/pair#", "/pair/#");
    expect(decodeOmegaBridgePairingText(`  ${link}\n`)).toEqual(decodedDesktopPayload);
  });

  test("rejects a link on any other origin, including a lookalike host", () => {
    const fragment = asPairingLink(desktopPayload).split("#")[1] ?? "";
    for (const base of [
      "https://openagents.com.evil.example/pair",
      "https://evil.example/pair",
      "http://openagents.com/pair",
      "https://openagents.com/pairing",
    ]) {
      expect(() => decodeOmegaBridgePairingText(`${base}#${fragment}`)).toThrow();
    }
  });

  test("rejects a pairing link whose fragment is not base64url JSON", () => {
    expect(() => decodeOmegaBridgePairingText(`${OMEGA_PAIRING_LINK_BASE}#%%%`)).toThrow(
      "The pairing link does not carry a readable payload.",
    );
  });

  test("rejects a desktop payload with the wrong bridge protocol", () => {
    expect(() =>
      decodeOmegaBridgePairingText(
        JSON.stringify({ ...desktopPayload, protocol: "openagents.omega.device_bridge.v2" }),
      ),
    ).toThrow();
  });

  test("rejects a desktop payload with an unknown extra field", () => {
    expect(() =>
      decodeOmegaBridgePairingText(JSON.stringify({ ...desktopPayload, extra: true })),
    ).toThrow();
  });

  test("rejects a desktop payload whose host name cannot form an endpoint", () => {
    expect(() =>
      decodeOmegaBridgePairingText(
        JSON.stringify({ ...desktopPayload, magicDnsName: "host/../evil" }),
      ),
    ).toThrow();
  });
});

describe("isOmegaPairingLink", () => {
  test("recognizes only the pairing base with a non-empty fragment", () => {
    expect(isOmegaPairingLink(asPairingLink(desktopPayload))).toBe(true);
    expect(isOmegaPairingLink(`${OMEGA_PAIRING_LINK_BASE}#`)).toBe(false);
    expect(isOmegaPairingLink(OMEGA_PAIRING_LINK_BASE)).toBe(false);
    expect(isOmegaPairingLink("https://openagents.com/#payload")).toBe(false);
    expect(isOmegaPairingLink(JSON.stringify(desktopPayload))).toBe(false);
  });
});
