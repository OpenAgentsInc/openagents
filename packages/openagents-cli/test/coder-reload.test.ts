import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { isSourceCheckout, RELOAD_EXIT_CODE, sourceCheckout } from "../src/coder-reload.js";

const directory = (manifest?: Record<string, unknown>, withSource = true): string => {
  const root = mkdtempSync(join(tmpdir(), "coder-reload-"));
  if (withSource) mkdirSync(join(root, "src"));
  if (manifest !== undefined) {
    writeFileSync(join(root, "package.json"), JSON.stringify(manifest));
  }
  return root;
};

describe("deciding whether a session can reload itself", () => {
  it("recognizes a checkout with sources and a build script", () => {
    expect(isSourceCheckout(directory({ scripts: { build: "tsc" } }))).toBe(true);
  });

  it("refuses a published install, which ships no sources", () => {
    // `/reload` there could only fail, so it reports instead of trying.
    expect(isSourceCheckout(directory({ scripts: { build: "tsc" } }, false))).toBe(false);
  });

  it("refuses a checkout with nothing to build with", () => {
    expect(isSourceCheckout(directory({ scripts: {} }))).toBe(false);
    expect(isSourceCheckout(directory({}))).toBe(false);
  });

  it("refuses a directory with no manifest, or an unreadable one", () => {
    expect(isSourceCheckout(directory())).toBe(false);
    const broken = directory();
    writeFileSync(join(broken, "package.json"), "{not json");
    expect(isSourceCheckout(broken)).toBe(false);
  });

  it("recognizes the checkout these tests run from", () => {
    // The case the command exists for.
    expect(sourceCheckout()).toBeDefined();
  });

  it("asks for a restart with a code no ordinary exit uses", () => {
    expect(RELOAD_EXIT_CODE).not.toBe(0);
    expect(RELOAD_EXIT_CODE).not.toBe(1);
    expect(RELOAD_EXIT_CODE).not.toBe(130);
  });
});
