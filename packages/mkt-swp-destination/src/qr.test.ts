import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";

import { parseDestination, type DestinationParseContext } from "./parse.js";
import { acceptScannedText, qrUnavailable } from "./qr.js";
import {
  DEFAULT_TEST_TIMESTAMP,
  encodeTestInvoice,
  encodeTestSegwitAddress,
} from "./testkit.js";

const context: DestinationParseContext = {
  rail: "chain",
  network: "regtest",
  nowSeconds: DEFAULT_TEST_TIMESTAMP + 60,
};

describe("QR intake", () => {
  test("a scan is exactly a paste: same parser, same result", () => {
    const payloads = [
      encodeTestSegwitAddress("regtest", 0, 20).toUpperCase(),
      encodeTestInvoice().toUpperCase(),
      `bitcoin:${encodeTestSegwitAddress("regtest", 0, 20)}?amount=0.001`,
    ];
    for (const payload of payloads) {
      expect(acceptScannedText(payload, context)).toEqual(
        parseDestination(payload, context),
      );
    }
  });

  test("scanned failures keep their discriminant", () => {
    const result = acceptScannedText("nonsense", context);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.failure.mode).toBe("unrecognized");
  });

  test("the default capability probes unavailable", async () => {
    await expect(Effect.runPromise(qrUnavailable.probe)).resolves.toBe(
      "unavailable",
    );
  });
});
