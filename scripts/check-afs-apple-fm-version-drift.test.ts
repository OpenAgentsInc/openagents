import { describe, expect, test } from "vite-plus/test";

import { inspectAppleFmVersionDrift } from "./check-afs-apple-fm-version-drift.ts";

/**
 * AFS-00 records the Apple FM bridge version drift as an intentional finding.
 * This test asserts the finding is present so the drift stays visible until
 * AFS-02 generates the staging pin from the single wire-version source. When
 * AFS-02 closes the drift, this test is updated to assert the sources agree.
 */
describe("AFS-00 Apple FM version-drift finding", () => {
  test("both version sources are readable", () => {
    const drift = inspectAppleFmVersionDrift();
    expect(drift.helperVersion).not.toBeNull();
    expect(drift.stagingVersion).not.toBeNull();
  });

  test("the current snapshot has the recorded 0.1.3 vs 0.1.1 drift", () => {
    const drift = inspectAppleFmVersionDrift();
    expect(drift.helperVersion).toBe("0.1.3");
    expect(drift.stagingVersion).toBe("0.1.1");
    expect(drift.agree).toBe(false);
    expect(drift.finding).toContain("0.1.3");
    expect(drift.finding).toContain("0.1.1");
  });
});
