#!/usr/bin/env node

import { execFile } from "node:child_process"
import { promisify } from "node:util"

const execFileAsync = promisify(execFile)

const status = await git(["status", "--porcelain=v1", "--untracked-files=all"])
const dirtyLines = status
  .split(/\r?\n/u)
  .map(line => line.trimEnd())
  .filter(line => line.length > 0)

if (dirtyLines.length > 0) {
  console.error("Khala Code read-only smoke verification failed: worktree changed.")
  for (const line of dirtyLines.slice(0, 50)) {
    console.error(line)
  }
  if (dirtyLines.length > 50) {
    console.error(`... ${dirtyLines.length - 50} more changed path(s) omitted`)
  }
  process.exit(1)
}

console.log("Khala Code read-only smoke verification passed: worktree clean.")

async function git(args: readonly string[]): Promise<string> {
  try {
    const { stdout } = await execFileAsync("git", [...args], { encoding: "utf8" })
    return stdout
  } catch (error) {
    const detail =
      typeof error === "object" && error !== null && "stderr" in error
        ? String(error.stderr).trim()
        : String(error)
    throw new Error(`git ${args.join(" ")} failed: ${detail}`)
  }
}
