/**
 * What the status line says about where the session is running.
 *
 * This reads the working directory directly rather than going through
 * `GitRunner`, which exposes clone, remote, and credential-helper operations
 * and has no general git call. When the delivered stage gives the session a
 * repository binding from the server, this is what it replaces.
 */

import { execFileSync } from "node:child_process";
import { basename } from "node:path";

export interface CoderWorkspace {
  readonly repository: string;
  readonly branch: string;
}

export function describeWorkspace(cwd: string = process.cwd()): CoderWorkspace {
  return {
    repository: basename(cwd),
    branch: git(["rev-parse", "--abbrev-ref", "HEAD"], cwd) ?? "no branch",
  };
}

function git(args: ReadonlyArray<string>, cwd: string): string | undefined {
  try {
    const out = execFileSync("git", [...args], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      timeout: 2000,
    });
    const trimmed = out.trim();
    return trimmed.length > 0 ? trimmed : undefined;
  } catch {
    // Not a repository, no git, or a timeout. The status line says so rather
    // than the command failing over a label.
    return undefined;
  }
}
