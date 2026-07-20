import { describe, expect, test } from "vite-plus/test";

import { inspectAppleFmVersionDrift } from "./check-afs-apple-fm-version-drift.ts";

/**
 * AFS-02 (#9080) closed the Apple FM bridge version drift that AFS-00 recorded.
 * The neutral `@openagentsinc/apple-fm-runtime` package is the single
 * wire-version source, the Swift bridge (Pylon + package copies) carries that
 * version, and the Desktop staging pin is derived from the package constant.
 * This test asserts every copy now agrees.
 */
describe("AFS-02 Apple FM version-drift reconciliation", () => {
  test("every version source is readable", () => {
    const drift = inspectAppleFmVersionDrift();
    expect(drift.canonicalVersion).not.toBeNull();
    expect(drift.helperVersion).not.toBeNull();
    expect(drift.packageNativeHelperVersion).not.toBeNull();
  });

  test("the single source, both Swift copies, and the derived staging pin agree", () => {
    const drift = inspectAppleFmVersionDrift();
    expect(drift.canonicalVersion).toBe("0.1.3");
    expect(drift.helperVersion).toBe("0.1.3");
    expect(drift.packageNativeHelperVersion).toBe("0.1.3");
    expect(drift.stagingDerivesFromSource).toBe(true);
    expect(drift.stagingVersion).toBe("0.1.3");
    expect(drift.agree).toBe(true);
    expect(drift.finding).toBeNull();
  });
});
