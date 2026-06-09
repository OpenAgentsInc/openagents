import { existsSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";

export function resolveProbeWorkspaceRoot(start = process.cwd()): string {
  let current = resolve(start);

  for (;;) {
    if (existsSync(resolve(current, "packages/runtime/src/cli.ts")) && existsSync(resolve(current, "README.md"))) {
      return current;
    }

    const parent = dirname(current);

    if (parent === current) {
      return resolve(start);
    }

    current = parent;
  }
}

export function resolveProbeChatWorkspaceRoot(
  env: Readonly<Record<string, string | undefined>> = {},
): string {
  return resolve(
    env.PROBE_WORKSPACE_ROOT ?? env.OPENAGENTS_WORKSPACE_ROOT ?? dirname(resolveProbeWorkspaceRoot()),
  );
}

export function resolveWorkspacePath(
  workspace: string,
  path: string,
): { readonly absolutePath: string; readonly relativePath: string } | undefined {
  const absolutePath = resolve(workspace, path);
  const relativePath = relative(workspace, absolutePath) || ".";

  if (
    path.length === 0 ||
    path.includes("\0") ||
    relativePath.startsWith("..") ||
    relativePath.split(sep).includes("..") ||
    relativePath.split(sep).includes(".git")
  ) {
    return undefined;
  }

  return { absolutePath, relativePath };
}
