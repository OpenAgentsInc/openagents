/**
 * The Hermes Web Crypto gap, kept closed.
 *
 * Found on an iOS Simulator during the omega#49 device proof, invisible to
 * every existing suite: Node has Web Crypto and the in-process test relay never
 * issues a NIP-42 challenge, so the whole owner-private room was unreachable on
 * a phone while the client's own tests stayed green. See
 * `../src/crypto-random-values.ts` for the two failures this closes.
 *
 * These tests drive the installer against synthetic scopes rather than the real
 * global, because the case that mattered on device was a scope with no `crypto`
 * namespace at all — a condition Node cannot reproduce in place.
 */
import { describe, expect, test } from "vite-plus/test";

import {
  installWebCryptoFallback,
  uuidV4FromBytes,
  type GetRandomValues,
  type WebCryptoScope,
} from "../src/crypto-random-values.ts";

/**
 * Deterministic stand-in for the platform CSPRNG.
 *
 * A plain byte counter is not good enough here: it repeats every 256 bytes,
 * which is only 16 UUIDs, and would make the freshness test fail on the stub
 * rather than on the code. This is a 32-bit LCG so the period is far longer
 * than anything a test draws.
 */
const countingRandomValues = (): GetRandomValues => {
  let state = 0x2545f491;
  return (<T extends ArrayBufferView>(array: T): T => {
    const bytes = new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
    for (let index = 0; index < bytes.length; index += 1) {
      state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
      bytes[index] = (state >>> 24) & 0xff;
    }
    return array;
  }) as GetRandomValues;
};

const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe("web crypto fallback", () => {
  test("installs both APIs onto a scope with no crypto namespace at all", () => {
    // Hermes' actual starting condition, and the one this exists for.
    const scope: WebCryptoScope = {};
    expect(installWebCryptoFallback(scope, countingRandomValues())).toEqual([
      "getRandomValues",
      "randomUUID",
    ]);
    expect(typeof scope.crypto?.getRandomValues).toBe("function");
    expect(typeof scope.crypto?.randomUUID).toBe("function");
  });

  test("getRandomValues fills the array it was handed and returns it", () => {
    const scope: WebCryptoScope = {};
    installWebCryptoFallback(scope, countingRandomValues());
    const array = new Uint8Array(32);
    expect(scope.crypto?.getRandomValues?.(array)).toBe(array);
    expect(array.every((byte) => byte !== 0)).toBe(true);
  });

  test("randomUUID produces a version 4, variant 1 UUID", () => {
    const scope: WebCryptoScope = {};
    installWebCryptoFallback(scope, countingRandomValues());
    expect(scope.crypto?.randomUUID?.()).toMatch(UUID_V4);
  });

  test("randomUUID draws fresh randomness for every call", () => {
    const scope: WebCryptoScope = {};
    installWebCryptoFallback(scope, countingRandomValues());
    const drawn = new Set(Array.from({ length: 64 }, () => scope.crypto?.randomUUID?.()));
    expect(drawn.size).toBe(64);
  });

  test("never displaces a real implementation", () => {
    // A platform that already has Web Crypto must be left exactly alone —
    // otherwise this file becomes the weakest randomness in the app.
    const real = countingRandomValues();
    const realUuid = (() => "11111111-1111-4111-8111-111111111111") as WebCryptoScope["crypto"] &
      object extends never
      ? never
      : () => `${string}-${string}-${string}-${string}-${string}`;
    const scope: WebCryptoScope = {
      crypto: { getRandomValues: real, randomUUID: realUuid },
    };
    expect(installWebCryptoFallback(scope, countingRandomValues())).toEqual([]);
    expect(scope.crypto?.getRandomValues).toBe(real);
    expect(scope.crypto?.randomUUID).toBe(realUuid);
  });

  test("installs only the half that is missing", () => {
    const real = countingRandomValues();
    const scope: WebCryptoScope = { crypto: { getRandomValues: real } };
    expect(installWebCryptoFallback(scope, countingRandomValues())).toEqual(["randomUUID"]);
    expect(scope.crypto?.getRandomValues).toBe(real);
  });

  test("the version and variant bits are set, not merely copied through", () => {
    const zeroed = uuidV4FromBytes(new Uint8Array(16));
    expect(zeroed).toBe("00000000-0000-4000-8000-000000000000");
    const ones = uuidV4FromBytes(new Uint8Array(16).fill(0xff));
    expect(ones).toBe("ffffffff-ffff-4fff-bfff-ffffffffffff");
  });

  test("refuses a byte count that cannot be a UUID", () => {
    expect(() => uuidV4FromBytes(new Uint8Array(15))).toThrow();
    expect(() => uuidV4FromBytes(new Uint8Array(17))).toThrow();
  });
});
