import { describe, expect, it } from "vitest";

import { activityPhrase, activityRenderers, activityRows } from "../src/coder-fleet.js";

describe("activity rows", () => {
  it("shows a shell command, truncated to the width", () => {
    expect(activityRows({ toolName: "bash", target: "pnpm test" }, 40)).toEqual([
      "bash: pnpm test",
    ]);
    expect(activityRows({ toolName: "shell", target: "pnpm build" }, 40)).toEqual([
      "shell: pnpm build",
    ]);

    const long = "a".repeat(200);
    const rows = activityRows({ toolName: "bash", target: long }, 20);
    expect(rows).toHaveLength(1);
    expect([...rows[0]!].length).toBeLessThanOrEqual(20);
    expect(rows[0]).toMatch(/^bash: /);
    expect(rows[0]).toMatch(/…$/);
  });

  it("shows a file path for read, write, and edit", () => {
    expect(activityRows({ toolName: "read", target: "src/a.ts" }, 40)).toEqual([
      "read: src/a.ts",
    ]);
    expect(activityRows({ toolName: "write", target: "src/b.ts" }, 40)).toEqual([
      "write: src/b.ts",
    ]);
    expect(activityRows({ toolName: "edit", target: "src/c.ts" }, 40)).toEqual([
      "edit: src/c.ts",
    ]);

    const long = "packages/openagents-cli/src/some/very/deep/file.ts";
    const rows = activityRows({ toolName: "read", target: long }, 20);
    expect(rows).toHaveLength(1);
    expect([...rows[0]!].length).toBeLessThanOrEqual(20);
  });

  it("shows a search pattern", () => {
    expect(activityRows({ toolName: "grep", target: "needle" }, 40)).toEqual(["grep: needle"]);
    expect(activityRows({ toolName: "search", target: "term" }, 40)).toEqual(["search: term"]);
    expect(activityRows({ toolName: "repo_grep", target: "pattern" }, 40)).toEqual([
      "repo_grep: pattern",
    ]);

    const long = "a very long search pattern that needs cutting";
    const rows = activityRows({ toolName: "grep", target: long }, 20);
    expect(rows).toHaveLength(1);
    expect([...rows[0]!].length).toBeLessThanOrEqual(20);
  });

  it("leaves the tool name alone when there is no target", () => {
    expect(activityRows({ toolName: "bash", target: undefined }, 40)).toEqual(["bash"]);
    expect(activityRows({ toolName: "read", target: undefined }, 40)).toEqual(["read"]);
    expect(activityRows({ toolName: "grep", target: undefined }, 40)).toEqual(["grep"]);
  });

  it("falls back to the exact activityPhrase for an unregistered tool", () => {
    const withTarget = { toolName: "think", target: "deeply" };
    expect(activityRows(withTarget, 80)).toEqual([activityPhrase(withTarget)]);

    const noTarget = { toolName: "unknown", target: undefined };
    expect(activityRows(noTarget, 80)).toEqual([activityPhrase(noTarget)]);
  });

  it("exposes the same renderer under harness aliases", () => {
    expect(activityRenderers["bash"]).toBe(activityRenderers["shell"]);
    expect(activityRenderers["write"]).toBe(activityRenderers["edit"]);
    expect(activityRenderers["grep"]).toBe(activityRenderers["search"]);
    expect(activityRenderers["repo_grep"]).toBe(activityRenderers["search"]);
  });

  it("shows a file read with a range", () => {
    expect(
      activityRows({ toolName: "read", target: "src/a.ts", meta: { range: { start: 10, end: 50 } } }, 40),
    ).toEqual(["read: src/a.ts:10-50"]);
  });

  it("shows a file write with a size", () => {
    expect(
      activityRows({ toolName: "write", target: "src/b.ts", meta: { size: 240 } }, 40),
    ).toEqual(["write: src/b.ts (240)"]);
  });

  it("shows a search with a hit count", () => {
    expect(
      activityRows({ toolName: "grep", target: "needle", meta: { hitCount: 3 } }, 40),
    ).toEqual(["grep: needle (3 hits)"]);
    expect(
      activityRows({ toolName: "search", target: "term", meta: { hitCount: 1 } }, 40),
    ).toEqual(["search: term (1 hit)"]);
    expect(
      activityRows({ toolName: "repo_grep", target: "pattern", meta: { hitCount: 0 } }, 40),
    ).toEqual(["repo_grep: pattern (0 hits)"]);
  });

  it("cuts a file or search with metadata to the given width", () => {
    const long = "packages/openagents-cli/src/some/very/deep/file.ts";
    const withRange = activityRows(
      { toolName: "read", target: long, meta: { range: { start: 0, end: 100 } } },
      20,
    );
    expect(withRange).toHaveLength(1);
    expect([...withRange[0]!].length).toBeLessThanOrEqual(20);
    expect(withRange[0]).toMatch(/^read: /);
  });
});
