import { createHash } from "crypto";
import { Effect } from "effect";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { ContainerError } from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const GITHUB_API = "https://api.github.com/repos/apple/container/releases/latest";
const CONTAINER_CLI = "container";
const DEFAULT_TIMEOUT_MS = 30_000;
const USER_AGENT = "openagents-sandbox-bootstrap/1.0";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

export interface BootstrapStatus {
  cliInstalled: boolean;
  cliVersion?: string;
  systemRunning: boolean;
  macOSVersion?: string;
  platform: string;
}

export interface BootstrapResult {
  success: boolean;
  message: string;
  installerPath?: string;
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Run command and get output
// ─────────────────────────────────────────────────────────────────────────────

const runCommand = (cmd: string, ...args: string[]) =>
  Effect.tryPromise({
    try: async () => {
      const proc = Bun.spawn([cmd, ...args], {
        stdout: "pipe",
        stderr: "pipe",
      });
      const stdout = await new Response(proc.stdout).text();
      const exitCode = await proc.exited;
      return { exitCode, stdout: stdout.trim() };
    },
    catch: (e) => new ContainerError("execution_failed", `Command failed: ${e}`),
  });

const fetchWithTimeout = async (
  url: string,
  init: RequestInit = {},
  timeoutMs: number = DEFAULT_TIMEOUT_MS
) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const resp = await fetch(url, {
      ...init,
      signal: controller.signal,
      headers: {
        "user-agent": USER_AGENT,
        ...(init.headers ?? {}),
      },
    });
    return resp;
  } finally {
    clearTimeout(timeout);
  }
};

const readSha256FromAsset = (contents: string): string | null => {
  const line = contents.split("\n").find((l) => l.trim().length > 0);
  if (!line) return null;
  const match = line.match(/[a-fA-F0-9]{64}/);
  return match ? match[0].toLowerCase() : null;
};

const cleanupDir = (fs: FileSystem.FileSystem, dir: string) =>
  fs.remove(dir, { recursive: true }).pipe(Effect.ignore);

// ─────────────────────────────────────────────────────────────────────────────
// Check Status
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Check the current status of the container runtime.
 */
export const checkStatus = Effect.gen(function* () {
  const status: BootstrapStatus = {
    cliInstalled: false,
    systemRunning: false,
    platform: process.platform,
  };

  // Check macOS version
  if (process.platform === "darwin") {
    const swVers = yield* Effect.either(runCommand("sw_vers", "-productVersion"));
    if (swVers._tag === "Right") {
      status.macOSVersion = swVers.right.stdout;
    }
  }

  // Check if container CLI is installed
  const which = yield* Effect.either(runCommand("which", CONTAINER_CLI));
  if (which._tag === "Right" && which.right.exitCode === 0) {
    status.cliInstalled = true;

    // Get version
    const version = yield* Effect.either(runCommand(CONTAINER_CLI, "--version"));
    if (version._tag === "Right" && version.right.exitCode === 0) {
      status.cliVersion = version.right.stdout;
    }

    // Check if system is running
    const systemStatus = yield* Effect.either(
      runCommand(CONTAINER_CLI, "system", "status"),
    );
    if (systemStatus._tag === "Right" && systemStatus.right.exitCode === 0) {
      status.systemRunning = true;
    }
  }

  return status;
});

// ─────────────────────────────────────────────────────────────────────────────
// Download Installer
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Download the latest container installer from GitHub.
 * Returns the path to the downloaded .pkg file.
 */
export const downloadInstaller = Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem;
  const path = yield* Path.Path;

  const cleanupOnError = (dir: string, error: unknown) =>
    cleanupDir(fs, dir).pipe(
      Effect.flatMap(() =>
        Effect.fail(
          error instanceof ContainerError
            ? error
            : new ContainerError("not_available", String(error))
        )
      )
    );

  // Get latest release info
  const response = yield* Effect.tryPromise({
    try: async () => {
      const resp = await fetchWithTimeout(GITHUB_API, { headers: { Accept: "application/json" } });
      if (!resp.ok) {
        throw new ContainerError("not_available", `Failed to fetch release info: HTTP ${resp.status}`);
      }
      return resp.json();
    },
    catch: (e) =>
      new ContainerError("not_available", `Failed to fetch release info: ${e}`),
  });

  // Find signed installer
  const assets = (response as any).assets ?? [];
  const installer = assets.find((a: any) =>
    a.name.includes("installer-signed.pkg"),
  );
  if (!installer) {
    return yield* Effect.fail(
      new ContainerError("not_available", "No signed installer found in release"),
    );
  }

  const checksumAsset = assets.find((a: any) => a.name.toLowerCase().includes("sha256"));
  if (!checksumAsset) {
    return yield* Effect.fail(
      new ContainerError("not_available", "No checksum asset found alongside installer"),
    );
  }

  const downloadUrl = installer.browser_download_url;
  const checksumUrl = checksumAsset.browser_download_url;
  const version = (response as any).tag_name ?? "unknown";

  // Create temp directory
  const tmpDir = path.join("/tmp", `container-installer-${Date.now()}`);
  yield* fs.makeDirectory(tmpDir, { recursive: true }).pipe(
    Effect.mapError((e) => new ContainerError("not_available", `Failed to create temp dir: ${e}`))
  );

  const pkgPath = path.join(tmpDir, `container-${version}.pkg`);
  const checksumPath = path.join(tmpDir, `container-${version}.sha256`);

  // Download the installer
  const downloadResult = yield* Effect.tryPromise({
    try: async () => {
      try {
        const resp = await fetchWithTimeout(downloadUrl);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const buffer = await resp.arrayBuffer();
        await Bun.write(pkgPath, buffer);
      } catch (e) {
        throw e;
      }

      const checksumResp = await fetchWithTimeout(checksumUrl);
      if (!checksumResp.ok) throw new Error(`HTTP ${checksumResp.status}`);
      const checksumText = await checksumResp.text();
      await Bun.write(checksumPath, checksumText);

      const expected = readSha256FromAsset(checksumText);
      if (!expected) {
        throw new ContainerError("not_available", "Checksum asset missing SHA-256 value");
      }

      const fileBuffer = await Bun.file(pkgPath).arrayBuffer();
      const actual = createHash("sha256").update(Buffer.from(fileBuffer)).digest("hex").toLowerCase();
      if (actual !== expected) {
        throw new ContainerError(
          "not_available",
          `Checksum mismatch for installer (expected ${expected}, got ${actual})`
        );
      }

      return {
        success: true,
        message: `Downloaded container ${version} installer`,
        installerPath: pkgPath,
      } satisfies BootstrapResult;
    },
    catch: (e) => e,
  });

  if (downloadResult instanceof ContainerError || downloadResult instanceof Error) {
    return yield* cleanupOnError(tmpDir, downloadResult);
  }

  return downloadResult satisfies BootstrapResult;
});

// ─────────────────────────────────────────────────────────────────────────────
// Install (requires user interaction for sudo)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Install the container CLI from a downloaded .pkg file.
 * This requires sudo and will prompt for password.
 */
export const installFromPkg = (pkgPath: string) =>
  Effect.gen(function* () {
    // Use open command to launch the installer GUI
    // This is more user-friendly than sudo installer
    const result = yield* Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["open", pkgPath], {
          stdout: "inherit",
          stderr: "inherit",
        });
        await proc.exited;
        return proc.exitCode;
      },
      catch: (e) =>
        new ContainerError("start_failed", `Failed to open installer: ${e}`),
    });

    return {
      success: result === 0,
      message:
        result === 0
          ? "Installer launched. Follow the prompts to complete installation."
          : "Failed to launch installer",
      installerPath: pkgPath,
    } satisfies BootstrapResult;
  });

/**
 * Install silently using sudo installer (for automation).
 * Requires the process to have sudo privileges.
 */
export const installSilent = (pkgPath: string) =>
  Effect.gen(function* () {
    const result = yield* Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(
          ["sudo", "installer", "-pkg", pkgPath, "-target", "/"],
          {
            stdout: "inherit",
            stderr: "inherit",
            stdin: "inherit",
          },
        );
        await proc.exited;
        return proc.exitCode;
      },
      catch: (e) =>
        new ContainerError("start_failed", `Installation failed: ${e}`),
    });

    if (result !== 0) {
      return yield* Effect.fail(
        new ContainerError("start_failed", `Installer exited with code ${result}`),
      );
    }

    return {
      success: true,
      message: "Container CLI installed successfully",
      installerPath: pkgPath,
    } satisfies BootstrapResult;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Start System Service
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Start the container system service.
 */
export const startSystem = Effect.gen(function* () {
  const result = yield* Effect.tryPromise({
    try: async () => {
      const proc = Bun.spawn([CONTAINER_CLI, "system", "start"], {
        stdout: "inherit",
        stderr: "inherit",
      });
      await proc.exited;
      return proc.exitCode;
    },
    catch: (e) =>
      new ContainerError("start_failed", `Failed to start system: ${e}`),
  });

  if (result !== 0) {
    return yield* Effect.fail(
      new ContainerError("start_failed", `System start exited with code ${result}`),
    );
  }

  return {
    success: true,
    message: "Container system service started",
  } satisfies BootstrapResult;
});

/**
 * Stop the container system service.
 */
export const stopSystem = Effect.gen(function* () {
  const result = yield* Effect.tryPromise({
    try: async () => {
      const proc = Bun.spawn([CONTAINER_CLI, "system", "stop"], {
        stdout: "inherit",
        stderr: "inherit",
      });
      await proc.exited;
      return proc.exitCode;
    },
    catch: (e) =>
      new ContainerError("start_failed", `Failed to stop system: ${e}`),
  });

  return {
    success: result === 0,
    message: result === 0 ? "Container system stopped" : "Failed to stop system",
  } satisfies BootstrapResult;
});

// ─────────────────────────────────────────────────────────────────────────────
// Full Bootstrap Flow
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Full bootstrap: check status, download if needed, install, and start.
 * This is the main entry point for setting up container support.
 */
export const bootstrap = Effect.gen(function* () {
  // Check current status
  const status = yield* checkStatus;

  // Must be on macOS
  if (status.platform !== "darwin") {
    return yield* Effect.fail(
      new ContainerError(
        "not_available",
        "Container CLI only available on macOS",
      ),
    );
  }

  // Check macOS version (need 26+)
  if (status.macOSVersion) {
    const majorVersion = parseInt(status.macOSVersion.split(".")[0], 10);
    if (majorVersion < 26) {
      return yield* Effect.fail(
        new ContainerError(
          "not_available",
          `macOS 26+ required, found ${status.macOSVersion}`,
        ),
      );
    }
  }

  // Already installed and running?
  if (status.cliInstalled && status.systemRunning) {
    return {
      success: true,
      message: `Container ${status.cliVersion} already installed and running`,
    } satisfies BootstrapResult;
  }

  // Need to install?
  if (!status.cliInstalled) {
    const download = yield* downloadInstaller;
    if (download.installerPath) {
      // Launch GUI installer
      yield* installFromPkg(download.installerPath);
      return {
        success: true,
        message:
          "Installer launched. After installation, run: container system start",
        installerPath: download.installerPath,
      } satisfies BootstrapResult;
    }
  }

  // Installed but not running - start it
  if (status.cliInstalled && !status.systemRunning) {
    yield* startSystem;
    return {
      success: true,
      message: "Container system service started",
    } satisfies BootstrapResult;
  }

  return {
    success: false,
    message: "Unknown state",
  } satisfies BootstrapResult;
});

// ─────────────────────────────────────────────────────────────────────────────
// CLI Runner
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Run bootstrap with Bun context provided.
 */
export const runBootstrap = () =>
  Effect.runPromise(bootstrap.pipe(Effect.provide(BunContext.layer)));

/**
 * Check status with Bun context provided.
 */
export const runCheckStatus = () =>
  Effect.runPromise(checkStatus.pipe(Effect.provide(BunContext.layer)));
