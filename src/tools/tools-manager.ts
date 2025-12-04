/**
 * Tools Manager - Auto-install helper binaries (rg/fd) for agent usage.
 *
 * Downloads and caches ripgrep and fd binaries per platform, exposing paths
 * to tools. Critical for headless MechaCoder runs where binaries may not be in PATH.
 */
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import * as Command from "@effect/platform/Command";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { Effect, Option } from "effect";
import * as nodeFs from "node:fs";
import * as nodePath from "node:path";
import { homedir, platform, arch } from "os";
import { join } from "path";

/** Supported tool names */
export type ToolName = "rg" | "fd";

/** Platform info for binary selection */
export interface PlatformInfo {
  os: "darwin" | "linux" | "windows";
  arch: "x64" | "arm64";
}

/** Tool download metadata */
interface ToolMeta {
  name: string;
  version: string;
  repo: string;
  getAssetName: (info: PlatformInfo) => string;
  getBinaryName: (info: PlatformInfo) => string;
}

const TOOL_METADATA: Record<ToolName, ToolMeta> = {
  rg: {
    name: "ripgrep",
    version: "14.1.1",
    repo: "BurntSushi/ripgrep",
    getAssetName: (info) => {
      const suffix = info.os === "windows" ? ".zip" : ".tar.gz";
      const archStr = info.arch === "arm64" ? "aarch64" : "x86_64";
      if (info.os === "darwin") {
        return `ripgrep-14.1.1-${archStr}-apple-darwin${suffix}`;
      } else if (info.os === "linux") {
        return `ripgrep-14.1.1-${archStr}-unknown-linux-musl${suffix}`;
      } else {
        return `ripgrep-14.1.1-${archStr}-pc-windows-msvc${suffix}`;
      }
    },
    getBinaryName: (info) => (info.os === "windows" ? "rg.exe" : "rg"),
  },
  fd: {
    name: "fd",
    version: "10.2.0",
    repo: "sharkdp/fd",
    getAssetName: (info) => {
      const suffix = info.os === "windows" ? ".zip" : ".tar.gz";
      const archStr = info.arch === "arm64" ? "aarch64" : "x86_64";
      if (info.os === "darwin") {
        return `fd-v10.2.0-${archStr}-apple-darwin${suffix}`;
      } else if (info.os === "linux") {
        return `fd-v10.2.0-${archStr}-unknown-linux-musl${suffix}`;
      } else {
        return `fd-v10.2.0-${archStr}-pc-windows-msvc${suffix}`;
      }
    },
    getBinaryName: (info) => (info.os === "windows" ? "fd.exe" : "fd"),
  },
};

/**
 * Detect the current platform info
 */
export const detectPlatform = (): PlatformInfo => {
  const p = platform();
  const a = arch();

  const os: PlatformInfo["os"] =
    p === "darwin" ? "darwin" : p === "win32" ? "windows" : "linux";
  const archType: PlatformInfo["arch"] = a === "arm64" ? "arm64" : "x64";

  return { os, arch: archType };
};

/**
 * Get the cache directory for tool binaries.
 * Uses ~/.openagents/bin by default.
 */
export const getCacheDir = (): string => join(homedir(), ".openagents", "bin");

/**
 * Get the path where a tool binary would be cached.
 */
export const getCachedToolPath = (tool: ToolName): string => {
  const info = detectPlatform();
  const meta = TOOL_METADATA[tool];
  return join(getCacheDir(), meta.getBinaryName(info));
};

/**
 * Check if a tool exists in the system PATH.
 */
export const findInPath = (
  tool: ToolName,
): Effect.Effect<
  Option.Option<string>,
  never,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const info = detectPlatform();
    const binaryName = TOOL_METADATA[tool].getBinaryName(info);
    const pathService = yield* Path.Path;
    const envPath = process.env.PATH ?? "";
    const segments = envPath.split(nodePath.delimiter).filter(Boolean);

    for (const segment of segments) {
      const candidate = pathService.join(segment, binaryName);
      try {
        const stat = nodeFs.statSync(candidate);
        if (stat.isFile()) {
          return Option.some(candidate);
        }
      } catch {
        // ignore and continue
      }
    }

    return Option.none<string>();
  });

/**
 * Check if a tool exists in the cache.
 */
export const findInCache = (
  tool: ToolName,
): Effect.Effect<Option.Option<string>, never, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const cachePath = getCachedToolPath(tool);

    const exists = yield* fs.exists(cachePath).pipe(
      Effect.orElse(() => Effect.succeed(false)),
    );

    return exists ? Option.some(cachePath) : Option.none<string>();
  });

/**
 * Get the download URL for a tool.
 */
export const getDownloadUrl = (tool: ToolName): string => {
  const info = detectPlatform();
  const meta = TOOL_METADATA[tool];
  const assetName = meta.getAssetName(info);
  return `https://github.com/${meta.repo}/releases/download/${meta.version}/${assetName}`;
};

/**
 * Download and extract a tool to the cache directory.
 */
export const downloadTool = (
  tool: ToolName,
): Effect.Effect<
  string,
  Error,
  FileSystem.FileSystem | Path.Path | CommandExecutor.CommandExecutor
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const pathService = yield* Path.Path;
    const executor = yield* CommandExecutor.CommandExecutor;

    const info = detectPlatform();
    const meta = TOOL_METADATA[tool];
    const url = getDownloadUrl(tool);
    const cacheDir = getCacheDir();
    const targetPath = getCachedToolPath(tool);

    // Ensure cache directory exists
    yield* fs
      .makeDirectory(cacheDir, { recursive: true })
      .pipe(Effect.orElse(() => Effect.void));

    // Create temp directory for download
    const tempDir = yield* fs.makeTempDirectory({ prefix: `${tool}-download` });

    const assetName = meta.getAssetName(info);
    const archivePath = pathService.join(tempDir, assetName);

    // Download the archive using curl
    const curlCmd = Command.make("curl", "-L", "-o", archivePath, url);
    yield* Effect.scoped(
      Effect.gen(function* () {
        const proc = yield* Effect.acquireRelease(
          executor.start(curlCmd),
          (p) =>
            p.isRunning.pipe(
              Effect.flatMap((running) =>
                running ? p.kill("SIGKILL") : Effect.void,
              ),
              Effect.orElse(() => Effect.void),
            ),
        );

        const exitCode = yield* proc.exitCode;
        if (Number(exitCode) !== 0) {
          return yield* Effect.fail(new Error(`Failed to download ${tool} from ${url}`));
        }
      }),
    );

    // Extract the archive
    if (info.os === "windows") {
      // Use PowerShell to extract ZIP on Windows
      const extractCmd = Command.make(
        "powershell",
        "-Command",
        `Expand-Archive -Path '${archivePath}' -DestinationPath '${tempDir}'`,
      );
      yield* Effect.scoped(
        Effect.gen(function* () {
          const proc = yield* Effect.acquireRelease(
            executor.start(extractCmd),
            (p) =>
              p.isRunning.pipe(
                Effect.flatMap((running) =>
                  running ? p.kill("SIGKILL") : Effect.void,
                ),
                Effect.orElse(() => Effect.void),
              ),
          );

          const exitCode = yield* proc.exitCode;
          if (Number(exitCode) !== 0) {
            return yield* Effect.fail(new Error(`Failed to extract ${tool}`));
          }
        }),
      );
    } else {
      // Use tar on Unix
      const extractCmd = Command.make("tar", "-xzf", archivePath, "-C", tempDir);
      yield* Effect.scoped(
        Effect.gen(function* () {
          const proc = yield* Effect.acquireRelease(
            executor.start(extractCmd),
            (p) =>
              p.isRunning.pipe(
                Effect.flatMap((running) =>
                  running ? p.kill("SIGKILL") : Effect.void,
                ),
                Effect.orElse(() => Effect.void),
              ),
          );

          const exitCode = yield* proc.exitCode;
          if (Number(exitCode) !== 0) {
            return yield* Effect.fail(new Error(`Failed to extract ${tool}`));
          }
        }),
      );
    }

    // Find the binary in the extracted content
    const binaryName = meta.getBinaryName(info);
    const findBinary = (dir: string): Effect.Effect<string | null, never, FileSystem.FileSystem> =>
      Effect.gen(function* () {
        const entries = yield* fs
          .readDirectory(dir)
          .pipe(Effect.orElse(() => Effect.succeed([] as string[])));

        for (const entry of entries) {
          const fullPath = pathService.join(dir, entry);
          const stat = yield* fs.stat(fullPath).pipe(
            Effect.orElse(() => Effect.succeed(null)),
          );

          if (!stat) continue;

          if (stat.type === "File" && entry === binaryName) {
            return fullPath;
          }

          if (stat.type === "Directory") {
            const found = yield* findBinary(fullPath);
            if (found) return found;
          }
        }

        return null;
      });

    const sourcePath = yield* findBinary(tempDir);
    if (!sourcePath) {
      return yield* Effect.fail(
        new Error(`Could not find ${binaryName} in extracted archive`),
      );
    }

    // Copy to cache location
    yield* fs.copyFile(sourcePath, targetPath);

    // Make executable on Unix
    if (info.os !== "windows") {
      const chmodCmd = Command.make("chmod", "+x", targetPath);
      yield* Effect.scoped(
        Effect.gen(function* () {
          const proc = yield* Effect.acquireRelease(
            executor.start(chmodCmd),
            (p) =>
              p.isRunning.pipe(
                Effect.flatMap((running) =>
                  running ? p.kill("SIGKILL") : Effect.void,
                ),
                Effect.orElse(() => Effect.void),
              ),
          );

          yield* proc.exitCode;
        }),
      );
    }

    // Cleanup temp directory
    yield* fs.remove(tempDir, { recursive: true }).pipe(
      Effect.orElse(() => Effect.void),
    );

    return targetPath;
  });

/**
 * Ensure a tool is available, returning its path.
 * Checks cache first, then PATH, then downloads if needed.
 */
export const ensureTool = (
  tool: ToolName,
): Effect.Effect<
  string,
  Error,
  FileSystem.FileSystem | Path.Path | CommandExecutor.CommandExecutor
> =>
  Effect.gen(function* () {
    // First check cache
    const cachedPath = yield* findInCache(tool);
    if (Option.isSome(cachedPath)) {
      return cachedPath.value;
    }

    // Then check PATH
    const pathLocation = yield* findInPath(tool);
    if (Option.isSome(pathLocation)) {
      return pathLocation.value;
    }

    // Download and install
    return yield* downloadTool(tool);
  });

/**
 * Get the path to a tool, returning null if not found (no auto-download).
 */
export const getToolPath = (
  tool: ToolName,
): Effect.Effect<
  string | null,
  never,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    // First check cache
    const cachedPath = yield* findInCache(tool);
    if (Option.isSome(cachedPath)) {
      return cachedPath.value;
    }

    // Then check PATH
    const pathLocation = yield* findInPath(tool);
    if (Option.isSome(pathLocation)) {
      return pathLocation.value;
    }

    return null;
  });
