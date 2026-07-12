import { execFileSync } from "node:child_process"
import { workspaceGitEnvironment } from "../src/git-process-environment.ts"

export function isolatedGitEnvironment(
  environment: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
  return workspaceGitEnvironment(environment)
}

export function runGitFixture(
  root: string,
  args: ReadonlyArray<string>,
): string {
  return execFileSync("git", [...args], {
    cwd: root,
    encoding: "utf8",
    env: isolatedGitEnvironment(),
    stdio: ["ignore", "pipe", "pipe"],
  })
}
