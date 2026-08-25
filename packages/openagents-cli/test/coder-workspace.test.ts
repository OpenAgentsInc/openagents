import { describe, expect, it } from "vitest";
import { describeWorkspace } from "../src/coder-workspace.js";
import { resolve } from "node:path";

describe("describeWorkspace", () => {
  it("reports the full absolute path as repository", () => {
    const cwd = process.cwd();
    const workspace = describeWorkspace(cwd);
    expect(workspace.repository).toBe(resolve(cwd));
  });

  it("resolves relative cwd input to absolute path", () => {
    const workspace = describeWorkspace(".");
    expect(workspace.repository).toBe(resolve("."));
  });
});
