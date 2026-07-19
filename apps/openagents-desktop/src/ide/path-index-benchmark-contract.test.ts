import { readFileSync } from "node:fs";
import path from "node:path";

import { Exit, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdePathIndexDeliveryReceiptSchema,
  IdePathIndexPackagedJourneyReceiptSchema,
} from "./index-benchmark-contract.ts";

const receiptPath = path.resolve(
  import.meta.dirname,
  "../../benchmarks/ide/2026-07-19-ide-02-path-index.json",
);
const packagedReceiptPath = path.resolve(
  import.meta.dirname,
  "../../benchmarks/ide/2026-07-19-ide-02-packaged-journey.json",
);

describe("IDE-02 path-index benchmark receipt", () => {
  test("decodes the generated receipt and preserves every release gate", () => {
    const raw: unknown = JSON.parse(readFileSync(receiptPath, "utf8"));
    const exit = Schema.decodeUnknownExit(IdePathIndexDeliveryReceiptSchema)(raw);
    expect(Exit.isSuccess(exit)).toBe(true);
    if (!Exit.isSuccess(exit)) return;
    expect(exit.value.fixtureFiles).toBe(10_000);
    expect(exit.value.fixtureDirectories).toBe(100);
    expect(exit.value.metrics.every(metric => metric.passed)).toBe(true);
    expect(exit.value.journeys.every(journey => journey.passed)).toBe(true);
    expect(exit.value.resources.sourceSubscriptionCountAfter).toBe(0);
    expect(exit.value.resources.stoppedAccessRefused).toBe(true);
    expect(exit.value.placement.map(decision => decision._tag)).toEqual(["Select", "Reject"]);
  });

  test("decodes the packaged large-repository pointer and keyboard journey", () => {
    const raw: unknown = JSON.parse(readFileSync(packagedReceiptPath, "utf8"));
    const exit = Schema.decodeUnknownExit(IdePathIndexPackagedJourneyReceiptSchema)(raw);
    expect(Exit.isSuccess(exit)).toBe(true);
    if (!Exit.isSuccess(exit)) return;
    expect(exit.value.packaged).toBe(true);
    expect(exit.value.indexedNodes).toBeGreaterThan(5_000);
    expect(exit.value.pointerActivation).toBe(true);
    expect(exit.value.keyboardHomeEnd).toBe(true);
    expect(exit.value.keyboardContextMenu).toBe(true);
    expect(exit.value.rootWithheld).toBe(true);
  });
});
