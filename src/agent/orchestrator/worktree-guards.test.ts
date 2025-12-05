import { describe, expect, test } from "bun:test";
import { createWorktreeGuardHook } from "./worktree-guards.js";

const worktreePath = "/repo/.worktrees/task-123";

const makeInput = (toolName: string, filePath: string) => ({
  hook_event_name: "PreToolUse",
  tool_name: toolName,
  tool_input: { file_path: filePath },
  cwd: worktreePath,
});

describe("createWorktreeGuardHook", () => {
  test("blocks file operations outside the worktree for core tools", async () => {
    const guard = createWorktreeGuardHook(worktreePath) as any;
    const outsidePath = "../outside.txt";

    for (const tool of ["Read", "Edit", "Write"]) {
      const result = (await guard(makeInput(tool, outsidePath) as any)) as any;
      expect(result.continue).toBe(false);
      expect(result.result?.text).toContain("WORKTREE ISOLATION VIOLATION");
      expect(result.result?.text).toContain(outsidePath);
    }
  });

  test("allows file operations inside the worktree", async () => {
    const guard = createWorktreeGuardHook(worktreePath) as any;
    const result = (await guard(makeInput("Edit", "src/index.ts") as any)) as any;
    expect(result.continue).toBe(true);
  });
});
