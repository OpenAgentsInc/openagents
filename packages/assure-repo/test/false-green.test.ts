import { describe, expect, test } from "vite-plus/test";

import { classifyTestSource, scannableTestFiles } from "../src/index.ts";

describe("classifyTestSource — coverage theater", () => {
  test("flags a test block with no assertion", () => {
    const source = `test("does work", () => {\n  doTheThing()\n})\n`;
    const candidates = classifyTestSource("a.test.ts", source);
    expect(candidates.some((c) => c.mode === "false_green_coverage_theater")).toBe(true);
  });

  test("does NOT flag a test that delegates to an expect* helper", () => {
    const source = `test("ok", () => {\n  expectLockout(await server.fetch(req), "x")\n})\n`;
    const candidates = classifyTestSource("a.test.ts", source);
    expect(candidates.some((c) => c.mode === "false_green_coverage_theater")).toBe(false);
  });

  test("does NOT flag a test with a matcher chain", () => {
    const source = `test("ok", () => {\n  expect(f()).toEqual([1,2,3])\n})\n`;
    expect(
      classifyTestSource("a.test.ts", source).some(
        (c) => c.mode === "false_green_coverage_theater",
      ),
    ).toBe(false);
  });

  test("does NOT flag node assert.method calls", () => {
    const source = `test("ok", () => {\n  assert.equal(x, true)\n})\n`;
    expect(
      classifyTestSource("a.test.ts", source).some(
        (c) => c.mode === "false_green_coverage_theater",
      ),
    ).toBe(false);
  });

  test("is not fooled by a brace inside the test-name string", () => {
    const source = `test("reads {step,status} shape", () => {\n  expect(f()).toBe(1)\n})\n`;
    expect(
      classifyTestSource("a.test.ts", source).some(
        (c) => c.mode === "false_green_coverage_theater",
      ),
    ).toBe(false);
  });

  test("is not fooled by a leading options object", () => {
    const source = `test("slow", { timeout: 120000 }, () => {\n  assert.ok(g())\n})\n`;
    expect(
      classifyTestSource("a.test.ts", source).some(
        (c) => c.mode === "false_green_coverage_theater",
      ),
    ).toBe(false);
  });
});

describe("classifyTestSource — round-up and mocked-seam", () => {
  // These fixtures intentionally contain skip/only tokens as classifier input;
  // the ref keeps the audit that scans THIS file from flagging them. see #9066
  test("flags an untracked skip and any .only", () => {
    const source = `test.skip("later", () => { expect(1).toBe(1) })\nit.only("x", () => { expect(2).toBe(2) })\n`;
    const modes = classifyTestSource("a.test.ts", source).map((c) => c.mode);
    expect(modes.filter((m) => m === "false_green_round_up").length).toBe(2);
  });

  test("does NOT flag a skip that carries a tracking ref within 3 lines", () => {
    // tracked skip: the classifier should treat it as intentional. see #9066
    const source = `// deferred, tracked in #1234\ntest.skip("retired surface", () => { expect(1).toBe(1) })\n`;
    const rollups = classifyTestSource("a.test.ts", source).filter(
      (c) => c.mode === "false_green_round_up",
    );
    expect(rollups.length).toBe(0);
  });

  test("always flags .only even with a nearby ref (it disables other tests)", () => {
    const source = `// see #1234\nit.only("focused", () => { expect(1).toBe(1) })\n`;
    const rollups = classifyTestSource("a.test.ts", source).filter(
      (c) => c.mode === "false_green_round_up",
    );
    expect(rollups.length).toBe(1);
    expect(rollups[0]!.evidence).toContain("disables every other test");
  });

  test("flags a mocked module", () => {
    const source = `vi.mock("../src/seam.ts")\ntest("x", () => { expect(1).toBe(1) })\n`;
    expect(
      classifyTestSource("a.test.ts", source).some((c) => c.mode === "false_green_mocked_seam"),
    ).toBe(true);
  });
});

describe("scannableTestFiles", () => {
  test("keeps test files, drops conformance fixtures", () => {
    const files = scannableTestFiles([
      "packages/x/src/a.test.ts",
      "packages/y/conformance/valid/b.test.ts",
      "packages/z/src/c.ts",
    ]);
    expect(files).toEqual(["packages/x/src/a.test.ts"]);
  });
});
