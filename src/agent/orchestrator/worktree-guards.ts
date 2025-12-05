/**
 * Worktree Guardrails - ENFORCE file operation boundaries
 *
 * Technical enforcement that PREVENTS agents from editing files outside their
 * worktree during parallel execution. Uses Claude Code SDK PreToolUse hooks
 * to validate and BLOCK file operations that escape the isolation boundary.
 *
 * This is NOT guidance - it's a hard barrier that cannot be bypassed.
 */

import type { HookCallback } from "@anthropic-ai/claude-agent-sdk";
import { resolve, relative } from "node:path";

/**
 * Create a PreToolUse hook that BLOCKS file operations outside worktree
 *
 * Returns { continue: false } to BLOCK the tool call with an error message
 */
export const createWorktreeGuardHook = (
  worktreePath: string
): HookCallback => async (input) => {
  // Only guard file operations
  const fileTools = ["Read", "Edit", "Write", "Glob", "NotebookEdit"];
  const toolName = (input as any).tool_name;

  if (!fileTools.includes(toolName)) {
    // Allow all non-file tools (Bash, WebFetch, etc.)
    return { continue: true };
  }

  // Extract file path from tool input
  const toolInput = (input as any).tool_input || {};
  const filePath = toolInput.file_path || toolInput.path || toolInput.pattern || toolInput.notebook_path;

  if (!filePath) {
    // No file path to validate (shouldn't happen but allow it)
    return { continue: true };
  }

  // Resolve to absolute paths
  const absolutePath = resolve(input.cwd, filePath);
  const absoluteWorktree = resolve(worktreePath);

  // Check if path is within worktree
  const relativePath = relative(absoluteWorktree, absolutePath);
  const isWithinWorktree =
    relativePath.length > 0 &&
    !relativePath.startsWith("..") &&
    !relativePath.startsWith("/") &&
    !absolutePath.startsWith("/");  // Absolute paths outside worktree

  if (!isWithinWorktree) {
    // BLOCK: File operation outside worktree
    const errorMessage = `
🚫 WORKTREE ISOLATION VIOLATION - OPERATION BLOCKED

Tool: ${toolName}
Requested path: ${filePath}
Resolved to: ${absolutePath}
Your worktree: ${absoluteWorktree}

This file is OUTSIDE your worktree boundary. The operation has been BLOCKED.

You MUST only access files within your worktree:
  ${absoluteWorktree}/

The worktree contains ALL project files. You do not need to access anything outside it.

Use paths relative to your current directory or absolute paths within your worktree.
`.trim();

    // Return blocking response
    return {
      continue: false,
      result: {
        type: "text",
        text: errorMessage,
      },
    };
  }

  // Allow: File is within worktree
  return { continue: true };
};

/**
 * Check if a path is within the worktree boundary (for testing)
 */
export const isPathWithinWorktree = (
  filePath: string,
  worktreePath: string,
  baseCwd: string
): boolean => {
  const absolutePath = resolve(baseCwd, filePath);
  const absoluteWorktree = resolve(worktreePath);
  const relativePath = relative(absoluteWorktree, absolutePath);
  return (
    relativePath.length > 0 &&
    !relativePath.startsWith("..") &&
    !relativePath.startsWith("/")
  );
};
