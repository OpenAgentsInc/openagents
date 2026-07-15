import { describe, expect, test } from "vite-plus/test";
import { readFileSync } from "node:fs";

import {
  electronMvpAssuranceTarget,
  nativeSdkMvpAssuranceTarget,
} from "../scripts/mvp-assurance-target.ts";

const source = readFileSync(new URL("../scripts/run-mvp-assurance.ts", import.meta.url), "utf8");

describe("MVP assurance runner conversion contract", () => {
  test("binds the admitted run to Node 24, pnpm, and Vite Plus", () => {
    expect(source).toContain('runtime: "Node 24.13.1"');
    expect(electronMvpAssuranceTarget.criterion.environmentRef).toBe(
      "ENV-OA-DESKTOP-MVP-VITE-PLUS-1",
    );
    expect(electronMvpAssuranceTarget.criterion.adapterRef).toBe("openagents.vite_plus_test.v1");
    expect(electronMvpAssuranceTarget.fullGate.argv).toEqual([
      "pnpm",
      "--dir",
      "apps/openagents-desktop",
      "run",
      "verify",
    ]);
    expect(nativeSdkMvpAssuranceTarget.fullGate.hostGateFormat).toBe(
      "openagents.native-sdk.host-gate.v5",
    );
    expect(source).not.toContain("ENV-OA-DESKTOP-MVP-BUN");
    expect(source).not.toContain("openagents.bun_test");
    expect(source).toContain("OPENAGENTS_VITE_PLUS_TEST_ADAPTER_VERSION");
    expect(source).toContain("executeNativeSdkCriterionUnit");
    expect(source).toContain("normalizeNativeSdkHostGate");
    expect(source).toContain('publication: "withheld"');
    expect(source).not.toContain("Native SDK MVP assurance remains fail-closed");
    expect(source).not.toContain('includes("0 fail")');
  });
});
