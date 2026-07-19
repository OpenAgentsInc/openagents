import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { compareStrings } from "./schema.ts";

/**
 * Deterministic enumeration of the repository graph: git-tracked files, pnpm
 * workspace packages, Cargo crates, CLI bins, and release pipelines. No
 * network, no wall clock. Everything is derived from committed files so the
 * generator is reproducible.
 */

/**
 * Resolve the repository root even inside a shared worktree hub whose
 * `.git/config` sets `core.bare=true` (see issue #8984).
 */
export const repositoryRoot = (cwd: string = process.cwd()): string => {
  const out = execFileSync(
    "git",
    ["-c", "core.bare=false", "rev-parse", "--path-format=absolute", "--show-toplevel"],
    { cwd, encoding: "utf8" },
  );
  return out.trim();
};

/** All git-tracked files, sorted, repo-relative. */
export const trackedFiles = (root: string): ReadonlyArray<string> => {
  const out = execFileSync("git", ["-C", root, "ls-files", "--cached", "--exclude-standard"], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  return out
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .sort(compareStrings);
};

export type PnpmPackage = {
  readonly name: string;
  readonly path: string;
  readonly hasBin: boolean;
};

const readJson = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));

/** Parse the `packages:` globs from pnpm-workspace.yaml (bounded subset). */
export const workspaceGlobs = (root: string): ReadonlyArray<string> => {
  const file = join(root, "pnpm-workspace.yaml");
  if (!existsSync(file)) return [];
  const lines = readFileSync(file, "utf8").split("\n");
  const globs: string[] = [];
  let inPackages = false;
  for (const raw of lines) {
    const line = raw.replace(/\r$/, "");
    if (/^packages:\s*$/.test(line)) {
      inPackages = true;
      continue;
    }
    if (inPackages) {
      const match = /^\s+-\s+["']?([^"'#]+?)["']?\s*(#.*)?$/.exec(line);
      if (match) {
        globs.push(match[1]!.trim());
        continue;
      }
      if (/^\S/.test(line)) inPackages = false;
    }
  }
  return globs.sort(compareStrings);
};

/** Expand a single workspace glob to package directories that hold a package.json. */
const expandGlob = (
  root: string,
  glob: string,
  tracked: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  // Only the trailing `/*` and `/**` forms appear in this repo's workspace file.
  if (glob.endsWith("/**")) {
    const base = glob.slice(0, -3);
    return tracked
      .filter((f) => f.startsWith(`${base}/`) && f.endsWith("/package.json"))
      .map((f) => f.slice(0, -"/package.json".length));
  }
  if (glob.endsWith("/*")) {
    const base = glob.slice(0, -2);
    const dirs = new Set<string>();
    for (const f of tracked) {
      if (!f.startsWith(`${base}/`)) continue;
      const rest = f.slice(base.length + 1);
      const seg = rest.split("/")[0]!;
      if (existsSync(join(root, base, seg, "package.json"))) dirs.add(`${base}/${seg}`);
    }
    return [...dirs];
  }
  // Exact path (e.g. `apps/openagents.com`).
  return existsSync(join(root, glob, "package.json")) ? [glob] : [];
};

/** Enumerate every pnpm workspace package with a package.json. */
export const pnpmPackages = (
  root: string,
  tracked: ReadonlyArray<string>,
): ReadonlyArray<PnpmPackage> => {
  const dirs = new Set<string>();
  for (const glob of workspaceGlobs(root)) {
    for (const dir of expandGlob(root, glob, tracked)) dirs.add(dir);
  }
  const packages: PnpmPackage[] = [];
  for (const dir of dirs) {
    const pkgPath = join(root, dir, "package.json");
    if (!existsSync(pkgPath)) continue;
    let json: { name?: string; bin?: unknown } = {};
    try {
      json = readJson(pkgPath) as { name?: string; bin?: unknown };
    } catch {
      continue;
    }
    packages.push({
      name: json.name ?? dir,
      path: dir,
      hasBin: json.bin !== undefined,
    });
  }
  return packages.sort((a, b) => compareStrings(a.path, b.path));
};

export type CargoCrate = {
  readonly name: string;
  readonly path: string;
};

/** Parse `[workspace] members` from the root Cargo.toml (bounded subset). */
export const cargoCrates = (root: string): ReadonlyArray<CargoCrate> => {
  const file = join(root, "Cargo.toml");
  if (!existsSync(file)) return [];
  const text = readFileSync(file, "utf8");
  const workspaceMatch = /\[workspace\][\s\S]*?members\s*=\s*\[([\s\S]*?)\]/.exec(text);
  if (!workspaceMatch) return [];
  const members = [...workspaceMatch[1]!.matchAll(/["']([^"']+)["']/g)].map((m) => m[1]!);
  return members
    .filter((path) => existsSync(join(root, path, "Cargo.toml")))
    .map((path) => ({ name: path.split("/").pop() ?? path, path }))
    .sort((a, b) => compareStrings(a.path, b.path));
};

export type ReleasePipeline = {
  readonly name: string;
  readonly script: string;
};

/** Release/publish pipelines declared as root package.json scripts. */
export const releasePipelines = (root: string): ReadonlyArray<ReleasePipeline> => {
  const pkg = readJson(join(root, "package.json")) as { scripts?: Record<string, string> };
  const scripts = pkg.scripts ?? {};
  const pipelines: ReleasePipeline[] = [];
  for (const name of Object.keys(scripts).sort(compareStrings)) {
    if (/^(release|changelog|pack|qa:nightly)(:|$)/.test(name)) {
      pipelines.push({ name, script: scripts[name]! });
    }
  }
  return pipelines;
};
