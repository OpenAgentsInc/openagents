// Filesystem tool surface (computer-use), scoped to a run workspace.
//
// Reuses Probe's `resolveWorkspacePath` so reads/writes can never escape the run
// workspace (no `..`, no `.git`, no absolute breakout). Records named beats. The
// fs operations are injectable for deterministic tests (default: node:fs).

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { resolveWorkspacePath } from "../workspace";
import { type Timeline } from "./timeline";

export class FilesystemScopeError extends Error {
  constructor(path: string) {
    super(`path_escapes_workspace: ${path}`);
    this.name = "FilesystemScopeError";
  }
}

export interface FilesystemIo {
  readonly readFile: (absolutePath: string) => string;
  readonly writeFile: (absolutePath: string, contents: string) => void;
}

const defaultIo: FilesystemIo = {
  readFile: (p) => readFileSync(p, "utf8"),
  writeFile: (p, c) => {
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, c, "utf8");
  },
};

export interface FilesystemSurface {
  /** Read a file relative to the run workspace. Throws if it escapes scope. */
  readonly read: (path: string) => string;
  /** Write a file relative to the run workspace. Throws if it escapes scope. */
  readonly write: (path: string, contents: string) => void;
}

export interface MakeFilesystemSurfaceOptions {
  /** Absolute path of the run workspace root. */
  readonly workspace: string;
  readonly timeline: Timeline;
  readonly io?: FilesystemIo;
}

export function makeFilesystemSurface(options: MakeFilesystemSurfaceOptions): FilesystemSurface {
  const io = options.io ?? defaultIo;
  const { workspace, timeline } = options;
  const resolve = (path: string) => {
    const resolved = resolveWorkspacePath(workspace, path);
    if (resolved === undefined) throw new FilesystemScopeError(path);
    return resolved;
  };
  return {
    read: (path) => {
      const { absolutePath, relativePath } = resolve(path);
      const contents = io.readFile(absolutePath);
      timeline.beat({ surface: "filesystem", label: `read ${relativePath}`, detail: { path: relativePath } });
      return contents;
    },
    write: (path, contents) => {
      const { absolutePath, relativePath } = resolve(path);
      io.writeFile(absolutePath, contents);
      timeline.beat({
        surface: "filesystem",
        label: `write ${relativePath}`,
        // Only the path + byte length are public-safe; contents are withheld.
        detail: { path: relativePath, bytes: contents.length },
      });
    },
  };
}
