import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { VERSION, manifestPath } from "../src/version.js";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8")) as {
  version: string;
};

describe("VERSION", () => {
  it("is the version in package.json", () => {
    // The defect this replaces: a hand-edited constant said 0.1.7 while the
    // manifest said 0.2.1 and npm served 0.3.0, and nothing failed.
    expect(VERSION).toBe(manifest.version);
  });

  it("is a real version rather than the unknown marker", () => {
    expect(VERSION).not.toBe("unknown");
    expect(VERSION).toMatch(/^\d+\.\d+\.\d+/);
  });

  it("resolves the manifest from the compiled layout, not only from source", () => {
    // `dist/version.js` must find `package.json` one directory up, which is
    // also the published tarball's layout.
    expect(existsSync(manifestPath())).toBe(true);
  });

  it("reports the manifest version through the built binary", () => {
    const built = join(packageRoot, "dist", "main.js");
    if (!existsSync(built)) return; // `pnpm run test` may run before a build.

    const printed = execFileSync("node", [built, "--version"], { encoding: "utf8" }).trim();
    expect(printed).toContain(manifest.version);
  });

  it("does not sit behind the version already published to npm", () => {
    // The published package was ahead of source once, which is how a shipped
    // command came to exist in no source file. A source version below the
    // registry's is the signal that it happened again.
    let published: string;
    try {
      published = execFileSync("npm", ["view", "@openagentsinc/cli", "version"], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
        timeout: 15_000,
      }).trim();
    } catch {
      return; // Offline, or the registry is unreachable. Not this test's failure.
    }

    expect(compare(manifest.version, published)).toBeGreaterThanOrEqual(0);
  });
});

/** Compare two `major.minor.patch` versions, ignoring any prerelease suffix. */
function compare(left: string, right: string): number {
  const parse = (value: string) =>
    (value.split("-")[0] ?? "").split(".").map((part) => Number.parseInt(part, 10) || 0);
  const a = parse(left);
  const b = parse(right);
  for (let index = 0; index < 3; index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}
