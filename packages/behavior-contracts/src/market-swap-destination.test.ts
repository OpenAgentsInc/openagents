import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";

import { decodeBehaviorContractRegistryDocument } from "./contract";
import { checkBehaviorContractCoverage, fileOracleSourceLayer } from "./coverage.test-support";
import { marketSwapDestinationContractRegistry } from "./market-swap-destination";
import { validateBehaviorContractRegistry } from "./registry";

const repoRoot = resolve(import.meta.dirname, "../../..");

describe("market swap destination contract registry", () => {
  test("decodes and validates with zero issues", () => {
    const decoded = decodeBehaviorContractRegistryDocument(marketSwapDestinationContractRegistry);
    const validation = validateBehaviorContractRegistry(decoded);
    expect(validation.issues).toEqual([]);
    expect(validation.ok).toBe(true);
  });

  test("carries exactly the three laws issue #9317 requires", () => {
    expect(marketSwapDestinationContractRegistry.contracts.map((c) => c.contractId)).toEqual([
      "openagents_web.swap_destination.amountless_invoice_refused.v1",
      "openagents_web.swap_destination.invoice_amount_precedence.v1",
      "openagents_web.swap_destination.amount_edit_clears_invoice.v1",
    ]);
  });

  test("oracle coverage links against the real test sources on disk", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(marketSwapDestinationContractRegistry).pipe(
        Effect.provide(
          fileOracleSourceLayer(
            (path) => readFile(path, "utf8"),
            (ref) => resolve(repoRoot, ref),
          ),
        ),
      ),
    );
    expect(report.ok).toBe(true);
    for (const result of report.results) {
      expect(result.status).toBe("covered");
    }
  });
});
