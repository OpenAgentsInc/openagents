/**
 * Bounded, read-only workspace tool executors for the Apple FM tool loop
 * (AFM-4). These back the `read_file`, `list_files`, and `code_search` tools the
 * `apple-fm tool` CLI (and, later, the desktop) offer the local model.
 *
 * Every executor is TOTAL (`Effect<result, never>`): each returns a typed result
 * object, including an `error` field, rather than throwing. Safety is enforced
 * here, not trusted from the model:
 *   - the requested path is confined to the configured workspace root, both
 *     lexically AND by real-path containment, so `..`, absolute paths, and
 *     symlink escapes are refused;
 *   - a symlink leaf is refused outright;
 *   - output is capped (file bytes, directory entries, search matches/files) and
 *     marked `truncated` so a huge tree can never blow up the loop or a receipt.
 *
 * Results carry workspace-relative paths only; absolute paths never leave here.
 */

import { Effect } from "effect";
import { readdir, readFile, realpath, stat, lstat } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import type { AppleFmToolExecutor } from "./blueprint-tools.js";

export interface AppleFmWorkspaceToolCaps {
  readonly maxFileBytes: number;
  readonly maxListEntries: number;
  readonly maxSearchMatches: number;
  readonly maxSearchFiles: number;
  readonly maxSearchFileBytes: number;
}

export const DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS: AppleFmWorkspaceToolCaps = {
  maxFileBytes: 8_000,
  maxListEntries: 200,
  maxSearchMatches: 50,
  maxSearchFiles: 800,
  maxSearchFileBytes: 262_144,
};

const SKIP_DIRECTORIES = new Set([
  "node_modules",
  ".git",
  "dist",
  ".build",
  "out",
  "target",
  "coverage",
  ".next",
  ".turbo",
]);

export const APPLE_FM_WORKSPACE_TOOL_REFS = {
  readFile: "tool.probe.read_file",
  listFiles: "tool.probe.list_files",
  codeSearch: "tool.probe.code_search",
} as const;

/** Confine a requested relative path to the workspace, lexically + by realpath. */
async function confineToWorkspace(
  workspaceRoot: string,
  requested: string,
): Promise<{ readonly ok: true; readonly abs: string; readonly rel: string } | { readonly ok: false; readonly error: string }> {
  if (typeof requested !== "string" || requested.length === 0) {
    return { ok: false, error: "a relative path within the workspace is required" };
  }
  if (isAbsolute(requested)) {
    return { ok: false, error: "absolute paths are not allowed; use a workspace-relative path" };
  }
  const rootReal = await realpath(workspaceRoot).catch(() => resolve(workspaceRoot));
  const abs = resolve(rootReal, requested);
  const rel = relative(rootReal, abs);
  if (rel === "" || rel.startsWith("..") || rel.split(sep).includes("..")) {
    return { ok: false, error: "path escapes the workspace scope" };
  }
  // Real-path containment defeats symlink escapes for existing paths.
  const targetReal = await realpath(abs).catch(() => null);
  if (targetReal !== null) {
    const realRel = relative(rootReal, targetReal);
    if (realRel !== "" && (realRel.startsWith("..") || isAbsolute(realRel))) {
      return { ok: false, error: "path resolves outside the workspace scope (symlink escape refused)" };
    }
  }
  return { ok: true, abs, rel };
}

function stringArg(input: Readonly<Record<string, unknown>>, key: string): string | undefined {
  const value = input[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** Read a bounded UTF-8 file confined to the workspace. */
function makeReadFileExecutor(root: string, caps: AppleFmWorkspaceToolCaps): AppleFmToolExecutor {
  return (input) =>
    Effect.promise(async () => {
      const requested = stringArg(input, "path");
      if (requested === undefined) {
        return { tool: "read_file", error: "a 'path' argument is required" };
      }
      const confined = await confineToWorkspace(root, requested);
      if (!confined.ok) {
        return { tool: "read_file", path: requested, error: confined.error };
      }
      const info = await lstat(confined.abs).catch(() => null);
      if (info === null) {
        return { tool: "read_file", path: confined.rel, error: "file not found" };
      }
      if (info.isSymbolicLink()) {
        return { tool: "read_file", path: confined.rel, error: "symlinks are not allowed" };
      }
      if (!info.isFile()) {
        return { tool: "read_file", path: confined.rel, error: "not a regular file" };
      }
      const buffer = await readFile(confined.abs).catch((error: unknown) => error);
      if (!(buffer instanceof Buffer)) {
        return { tool: "read_file", path: confined.rel, error: `failed to read file: ${String(buffer)}` };
      }
      const truncated = buffer.byteLength > caps.maxFileBytes;
      const content = buffer.subarray(0, caps.maxFileBytes).toString("utf8");
      return { tool: "read_file", path: confined.rel, byteLength: buffer.byteLength, truncated, content };
    });
}

/** List the immediate entries of a bounded directory confined to the workspace. */
function makeListFilesExecutor(root: string, caps: AppleFmWorkspaceToolCaps): AppleFmToolExecutor {
  return (input) =>
    Effect.promise(async () => {
      const requested = stringArg(input, "path") ?? ".";
      const confined = await confineToWorkspace(root, requested === "." ? "." : requested).catch(() => null);
      // "." maps to the workspace root, which confineToWorkspace rejects as empty
      // rel; handle the root listing explicitly.
      const rootReal = await realpath(root).catch(() => resolve(root));
      const abs = requested === "." ? rootReal : confined && confined.ok ? confined.abs : null;
      const rel = requested === "." ? "." : confined && confined.ok ? confined.rel : null;
      if (abs === null || rel === null) {
        const error = confined && !confined.ok ? confined.error : "invalid directory path";
        return { tool: "list_files", path: requested, error };
      }
      const info = await lstat(abs).catch(() => null);
      if (info === null) {
        return { tool: "list_files", path: rel, error: "directory not found" };
      }
      if (info.isSymbolicLink()) {
        return { tool: "list_files", path: rel, error: "symlinks are not allowed" };
      }
      if (!info.isDirectory()) {
        return { tool: "list_files", path: rel, error: "not a directory" };
      }
      const dirents = await readdir(abs, { withFileTypes: true }).catch((error: unknown) => error);
      if (!Array.isArray(dirents)) {
        return { tool: "list_files", path: rel, error: `failed to list directory: ${String(dirents)}` };
      }
      const sorted = [...dirents].sort((a, b) => a.name.localeCompare(b.name));
      const truncated = sorted.length > caps.maxListEntries;
      const entries = sorted.slice(0, caps.maxListEntries).map((entry) => ({
        name: entry.name,
        kind: entry.isDirectory() ? "dir" : entry.isSymbolicLink() ? "symlink" : entry.isFile() ? "file" : "other",
      }));
      return { tool: "list_files", path: rel, entryCount: sorted.length, truncated, entries };
    });
}

/** Bounded literal, case-insensitive text search confined to the workspace. */
function makeCodeSearchExecutor(root: string, caps: AppleFmWorkspaceToolCaps): AppleFmToolExecutor {
  return (input) =>
    Effect.promise(async () => {
      const query = stringArg(input, "query");
      if (query === undefined) {
        return { tool: "code_search", error: "a 'query' argument is required" };
      }
      const scope = stringArg(input, "path") ?? ".";
      const rootReal = await realpath(root).catch(() => resolve(root));
      let scopeAbs = rootReal;
      let scopeRel = ".";
      if (scope !== ".") {
        const confined = await confineToWorkspace(root, scope);
        if (!confined.ok) {
          return { tool: "code_search", query, path: scope, error: confined.error };
        }
        scopeAbs = confined.abs;
        scopeRel = confined.rel;
      }
      const needle = query.toLowerCase();
      const matches: Array<{ path: string; line: number; preview: string }> = [];
      let filesScanned = 0;
      let truncated = false;

      const walk = async (dir: string): Promise<void> => {
        if (truncated || filesScanned >= caps.maxSearchFiles) {
          truncated = truncated || filesScanned >= caps.maxSearchFiles;
          return;
        }
        const dirents = await readdir(dir, { withFileTypes: true }).catch(() => [] as Array<import("node:fs").Dirent>);
        for (const entry of dirents) {
          if (matches.length >= caps.maxSearchMatches) {
            truncated = true;
            return;
          }
          if (entry.isSymbolicLink()) continue;
          const abs = join(dir, entry.name);
          if (entry.isDirectory()) {
            if (SKIP_DIRECTORIES.has(entry.name) || entry.name.startsWith(".")) continue;
            await walk(abs);
            if (truncated) return;
            continue;
          }
          if (!entry.isFile()) continue;
          filesScanned += 1;
          if (filesScanned > caps.maxSearchFiles) {
            truncated = true;
            return;
          }
          const info = await stat(abs).catch(() => null);
          if (info === null || info.size > caps.maxSearchFileBytes) continue;
          const buffer = await readFile(abs).catch(() => null);
          if (buffer === null) continue;
          if (buffer.subarray(0, 1024).includes(0)) continue; // skip binary
          const lines = buffer.toString("utf8").split("\n");
          for (let i = 0; i < lines.length; i += 1) {
            if (lines[i].toLowerCase().includes(needle)) {
              matches.push({ path: relative(rootReal, abs), line: i + 1, preview: lines[i].trim().slice(0, 200) });
              if (matches.length >= caps.maxSearchMatches) {
                truncated = true;
                return;
              }
            }
          }
        }
      };

      await walk(scopeAbs);
      return { tool: "code_search", query, path: scopeRel, matchCount: matches.length, filesScanned, truncated, matches };
    });
}

/**
 * The three bounded read-only executors keyed by Blueprint tool ref, ready to
 * hand to `projectProbeToolMenuToAppleFm({ executors })`.
 */
export function makeAppleFmWorkspaceReadOnlyExecutors(
  workspaceRoot: string,
  caps: AppleFmWorkspaceToolCaps = DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS,
): Record<string, AppleFmToolExecutor> {
  const root = resolve(workspaceRoot);
  return {
    [APPLE_FM_WORKSPACE_TOOL_REFS.readFile]: makeReadFileExecutor(root, caps),
    [APPLE_FM_WORKSPACE_TOOL_REFS.listFiles]: makeListFilesExecutor(root, caps),
    [APPLE_FM_WORKSPACE_TOOL_REFS.codeSearch]: makeCodeSearchExecutor(root, caps),
  };
}
