import { describe, expect, it } from "vitest";

import { shouldSkipHydratedPlaceholder } from "../../src/effuse-app/controllers/home/chatSession";

describe("apps/web home chat session helpers", () => {
  it("skips initial empty ready snapshot when hydrated state exists", () => {
    expect(
      shouldSkipHydratedPlaceholder({
        skippedHydratedPlaceholder: false,
        hasHydratedSnapshot: true,
        hydratedSnapshotMessageCount: 3,
        nextSnapshotMessageCount: 0,
        nextSnapshotStatus: "ready",
        nextSnapshotErrorText: null,
      }),
    ).toBe(true);
  });

  it("does not skip once already consumed or when snapshot carries signal", () => {
    expect(
      shouldSkipHydratedPlaceholder({
        skippedHydratedPlaceholder: true,
        hasHydratedSnapshot: true,
        hydratedSnapshotMessageCount: 3,
        nextSnapshotMessageCount: 0,
        nextSnapshotStatus: "ready",
        nextSnapshotErrorText: null,
      }),
    ).toBe(false);

    expect(
      shouldSkipHydratedPlaceholder({
        skippedHydratedPlaceholder: false,
        hasHydratedSnapshot: true,
        hydratedSnapshotMessageCount: 3,
        nextSnapshotMessageCount: 0,
        nextSnapshotStatus: "streaming",
        nextSnapshotErrorText: null,
      }),
    ).toBe(false);
  });
});
