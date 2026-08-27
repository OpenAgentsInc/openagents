import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8")) as {
  scripts: Record<string, string>;
};

describe("the repository completion gate", () => {
  test("checks Rust formatting", () => {
    expect(packageJson.scripts["fmt:check"]).toContain("cargo fmt --all -- --check");
  });

  test("runs the full Cargo workspace before the general test sweep", () => {
    expect(packageJson.scripts["test:rust"]).toBe("cargo test --workspace");
    expect(packageJson.scripts.check).toContain("pnpm run test:rust && pnpm run test");
    expect(packageJson.scripts["test:cloud-crates"]).toBeUndefined();
  });
});
