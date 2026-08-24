/**
 * Restarting a session on the code as it is now.
 *
 * The interface is developed by running it, and every change meant leaving the
 * session, rebuilding, and starting again -- which loses the conversation that
 * prompted the change. `/reload` does the same three things without the reader
 * doing them, and only where they mean something: a session running from a
 * source checkout.
 *
 * A published install has no `src` to build from and no build script to run, so
 * `/reload` there would be a command that could only fail. It reports that
 * instead of trying.
 */

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The exit code the interface uses to ask its runner for a restart. */
export const RELOAD_EXIT_CODE = 75;

/**
 * Whether this directory is something `/reload` could rebuild.
 *
 * Both have to be there. A published tarball ships `dist` and a manifest but no
 * `src`, and a manifest with no build script is nothing to rebuild with.
 */
export function isSourceCheckout(root: string): boolean {
  if (!existsSync(join(root, "src"))) return false;
  try {
    const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8")) as {
      scripts?: Record<string, unknown>;
    };
    return typeof manifest.scripts?.["build"] === "string";
  } catch {
    return false;
  }
}

/** The package this process is running out of, when it is a source checkout. */
export function sourceCheckout(): string | undefined {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  return isSourceCheckout(root) ? root : undefined;
}

export interface RebuildResult {
  readonly ok: boolean;
  /** What the compiler said, when it failed. */
  readonly output: string;
}

/**
 * Rebuild the checkout.
 *
 * The compiler's own output is carried back rather than a summary of it: a
 * reload that fails is a compile error the reader has to read, and "build
 * failed" tells them nothing they can act on.
 */
export function rebuild(root: string): RebuildResult {
  const result = spawnSync("pnpm", ["run", "build"], { cwd: root, encoding: "utf8" });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
  return { ok: result.status === 0, output };
}
