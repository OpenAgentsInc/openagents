import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import { decodeBehaviorContractRegistryDocument } from "./contract";
import { checkBehaviorContractCoverage, fileOracleSourceLayer } from "./coverage";
import { validateBehaviorContractRegistry } from "./registry";
import { marketSwapWidgetContractRegistry } from "./market-swap-widget";

const repoRoot = join(import.meta.dirname, "..", "..", "..");

describe("swap widget behavior contracts (SWAP-0 #9315)", () => {
  test("the registry decodes and validates", () => {
    const decoded = decodeBehaviorContractRegistryDocument(marketSwapWidgetContractRegistry);
    const validation = validateBehaviorContractRegistry(decoded);
    expect(validation).toEqual({ issues: [], ok: true });
    expect(decoded.contracts).toHaveLength(3);
    const ids = decoded.contracts.map((contract) => contract.contractId);
    expect(ids).toEqual([
      "openagents_web.swap_widget.primary_action_law.v1",
      "openagents_web.swap_widget.funding_gate.v1",
      "openagents_web.swap_widget.state_exhaustive_explanation.v1",
    ]);
    for (const contract of decoded.contracts) {
      expect(contract.state).toBe("enforced");
      expect(contract.enforcementTier).toBe("test-sweep");
    }
  });

  test("every enforced oracle resolves to a test file that names its contract", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(marketSwapWidgetContractRegistry).pipe(
        Effect.provide(
          fileOracleSourceLayer(
            (path) => readFile(path, "utf8"),
            (ref) => join(repoRoot, ref),
          ),
        ),
      ),
    );
    expect(report.results.filter((result) => result.status !== "covered")).toEqual([]);
    expect(report.ok).toBe(true);
  });
});
