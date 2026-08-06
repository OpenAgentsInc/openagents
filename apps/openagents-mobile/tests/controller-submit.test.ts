import { describe, expect, it } from "vite-plus/test";

import { createSubmissionGuard } from "../src/controller/submission-guard.ts";

describe("mobile controller submission guard", () => {
  it("admits exactly one send when the primary action is tapped twice", async () => {
    const guard = createSubmissionGuard();
    let release!: () => void;
    const blocked = new Promise<void>((resolve) => {
      release = resolve;
    });
    let effects = 0;
    const first = guard.run(async () => {
      effects += 1;
      await blocked;
      return "first";
    });
    const second = guard.run(async () => {
      effects += 1;
      return "second";
    });
    expect(guard.phase()).toBe("submitting");
    expect(await second).toBeUndefined();
    release();
    expect(await first).toBe("first");
    expect(effects).toBe(1);
    expect(guard.phase()).toBe("idle");
  });
});
