import { describe, expect, it } from "bun:test";
import { buildHint, getSuiteMode } from "./hints.js";

describe("getSuiteMode", () => {
  it("detects fm-mini from path", () => {
    expect(getSuiteMode("tasks/terminal-bench-mini.json")).toBe("fm-mini");
  });

  it("detects tb2 from path", () => {
    expect(getSuiteMode("tasks/terminal-bench-2.json")).toBe("tb2");
  });

  it("returns unknown for other paths", () => {
    expect(getSuiteMode("tasks/custom-suite.json")).toBe("unknown");
  });
});

describe("buildHint", () => {
  it("returns undefined for tb2 mode", () => {
    const hint = buildHint("read file foo.txt and copy to bar.txt", [], "tb2");
    expect(hint).toBeUndefined();
  });

  it("returns hint for fm-mini read task", () => {
    const hint = buildHint("read file foo.txt and copy to bar.txt", [], "fm-mini");
    expect(hint).toContain("read_file");
  });

  it("returns undefined for unknown mode", () => {
    const hint = buildHint("read file foo.txt", [], "unknown");
    expect(hint).toBeUndefined();
  });
});
