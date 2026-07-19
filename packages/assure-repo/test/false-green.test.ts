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
  test("flags skip/todo/only", () => {
    const source = `test.skip("later", () => { expect(1).toBe(1) })\nit.only("x", () => { expect(2).toBe(2) })\n`;
    const modes = classifyTestSource("a.test.ts", source).map((c) => c.mode);
    expect(modes.filter((m) => m === "false_green_round_up").length).toBe(2);
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
