/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- receipt decoding is synchronous. */
import { readFile } from "node:fs/promises";

import { describe, expect, test } from "vite-plus/test";

import {
  decodeOmegaMobileMirrorJourneyReceipt,
  OMEGA_MOBILE_MIRROR_JOURNEY_SCHEMA,
} from "../src/workroom/omega-mobile-mirror-journey";

describe("TM-06 mirror journey receipt", () => {
  test("keeps simulator truth distinct from blocked live and physical evidence", async () => {
    const fixture = JSON.parse(
      await readFile(
        new URL(
          "../../../docs/mobile/evidence/2026-07-27-omega-mobile-mirror-simulator.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ) as unknown;
    const receipt = decodeOmegaMobileMirrorJourneyReceipt(fixture);

    expect(receipt.schema).toBe(OMEGA_MOBILE_MIRROR_JOURNEY_SCHEMA);
    expect(receipt.stages.map((stage) => stage.stage)).toEqual(["M0", "M1", "M2", "revocation"]);
    expect(receipt.stages.every((stage) => stage.status === "passed_simulator")).toBe(true);
    expect(receipt.host).toMatchObject({
      dependenciesLanded: true,
      liveJourneyRun: false,
    });
    expect(receipt.residual.map((stage) => stage.status)).toEqual(["not_run"]);
    expect(receipt.summary.overall).toBe("passed_simulator");
  });

  test("rejects secret-shaped evidence", async () => {
    const fixture = JSON.parse(
      await readFile(
        new URL(
          "../../../docs/mobile/evidence/2026-07-27-omega-mobile-mirror-simulator.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ) as {
      residual: Array<{ summary: string }>;
    };
    const residual = fixture.residual.at(0);
    if (residual === undefined) throw new Error("The receipt has no residual row.");
    residual.summary = "pairingSecret must never be recorded";

    expect(() => decodeOmegaMobileMirrorJourneyReceipt(fixture)).toThrow();
  });
});
