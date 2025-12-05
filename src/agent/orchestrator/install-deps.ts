/**
 * Dependency Installation Helper
 *
 * Shared helper for installing dependencies in worktrees and sandboxes.
 */
import type { ProjectConfig } from "../../tasks/schema.js";

export interface InstallSettings {
  args: string[];
  timeoutMs: number;
  skipInstall: boolean;
}

export interface InstallResult {
  success: boolean;
  error?: string;
  timedOut?: boolean;
}

export const getInstallSettings = (projectConfig: ProjectConfig): InstallSettings => {
  const parallel = projectConfig.parallelExecution;
  const args =
    parallel?.installArgs && parallel.installArgs.length > 0
      ? parallel.installArgs
      : ["--frozen-lockfile"];
  const timeoutMs = parallel?.installTimeoutMs ?? 15 * 60 * 1000;
  const skipInstall = args.includes("--skip-install");
  return {
    args: args.filter((arg) => arg !== "--skip-install"),
    timeoutMs,
    skipInstall,
  };
};

export const getInstallSettingsFromOptions = (options: {
  installArgs?: string[];
  installTimeoutMs?: number;
}): InstallSettings => {
  const args =
    options.installArgs && options.installArgs.length > 0
      ? options.installArgs
      : ["--frozen-lockfile"];
  const timeoutMs = options.installTimeoutMs ?? 15 * 60 * 1000;
  const skipInstall = args.includes("--skip-install");
  return {
    args: args.filter((arg) => arg !== "--skip-install"),
    timeoutMs,
    skipInstall,
  };
};

export const installDeps = async (
  cwd: string,
  settings: InstallSettings,
): Promise<InstallResult> => {
  if (settings.skipInstall) {
    return { success: true };
  }

  let timedOut = false;
  const proc = Bun.spawn(["bun", "install", ...settings.args], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });

  const timer = setTimeout(() => {
    timedOut = true;
    proc.kill();
  }, settings.timeoutMs);

  await proc.exited;
  clearTimeout(timer);

  if (timedOut) {
    return {
      success: false,
      error: `bun install timed out after ${Math.floor(settings.timeoutMs / 1000)}s`,
      timedOut: true,
    };
  }

  if (proc.exitCode !== 0) {
    const stderr = await new Response(proc.stderr).text();
    return {
      success: false,
      error: `bun install failed: ${stderr}`,
    };
  }

  return { success: true };
};
