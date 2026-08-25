import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// The vendor script is the single definition of the transform; the guard
// re-runs it and compares, so the vendored tree cannot drift from the
// canonical packages silently.
// eslint-disable-next-line @typescript-eslint/no-unsafe-assignment
const { VENDORED, renderVendored } = (await import("../scripts/vendor-memory.mjs")) as unknown as {
  VENDORED: ReadonlyArray<readonly [string, string, ReadonlyArray<readonly [string, string]>]>;
  renderVendored: (source: string, rewrites: ReadonlyArray<readonly [string, string]>) => string;
};

const memoryDir = join(__dirname, "..", "src", "memory");

describe("the vendored memory tree", () => {
  it("matches the canonical packages exactly, import rewrites aside", () => {
    for (const [source, name, rewrites] of VENDORED) {
      const vendored = readFileSync(join(memoryDir, name), "utf8");
      expect(vendored, `${name} drifted from packages/${source}`).toBe(
        renderVendored(source, rewrites),
      );
    }
  });
});
