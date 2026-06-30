#!/usr/bin/env bun

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
  const proc = Bun.spawn(["git", ...args], {
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${stderr.trim() || `exit ${exitCode}`}`)
  }
  return stdout
}
