import { createRequire } from "node:module";
import { execSync } from "node:child_process";

export interface ClaudeCodeAvailability {
  available: boolean;
  version?: string;
  cliPath?: string;
  reason?: string;
}

export interface DetectClaudeCodeOptions {
  /** Whether to run an optional health check (defaults to false) */
  healthCheck?: boolean;
  /** Custom health check (used in tests to avoid network calls) */
  healthCheckFn?: () => Promise<void>;
  /** Custom SDK resolver (used in tests to simulate presence/absence) */
  sdkResolver?: () => Promise<{ version?: string }>;
  /** Custom CLI checker (used in tests) */
  cliChecker?: () => { available: boolean; path?: string };
}

const defaultSdkResolver = async (): Promise<{ version?: string }> => {
  const require = createRequire(import.meta.url);
  const pkg = require("@anthropic-ai/claude-agent-sdk/package.json") as { version?: string };
  return pkg.version ? { version: pkg.version } : {};
};

const defaultHealthCheck = async (): Promise<void> => {
  const sdk = await import("@anthropic-ai/claude-agent-sdk");
  if (typeof (sdk as any).query !== "function") {
    throw new Error("query export not found");
  }
};

const defaultCliChecker = (): { available: boolean; path?: string } => {
  try {
    const path = execSync("which claude", { encoding: "utf-8" }).trim();
    return { available: true, path };
  } catch {
    return { available: false };
  }
};

/**
 * Detect whether Claude Code/Agent SDK is available for use.
 * Checks for claude CLI binary (authenticates via Claude Max subscription).
 */
export const detectClaudeCode = async (
  options?: DetectClaudeCodeOptions
): Promise<ClaudeCodeAvailability> => {
  const resolveSdk = options?.sdkResolver ?? defaultSdkResolver;
  const checkCli = options?.cliChecker ?? defaultCliChecker;

  // Check for claude CLI binary
  const cliResult = checkCli();
  if (!cliResult.available) {
    return {
      available: false,
      reason: "Claude CLI not found. Install: npm install -g @anthropic-ai/claude-code",
    };
  }

  // Check SDK is installed
  let version: string | undefined;
  try {
    const sdk = await resolveSdk();
    version = sdk.version;
  } catch {
    const unavailable: ClaudeCodeAvailability = {
      available: false,
      reason: "SDK not installed: bun add -E @anthropic-ai/claude-agent-sdk",
    };
    if (cliResult.path) unavailable.cliPath = cliResult.path;
    return unavailable;
  }

  if (options?.healthCheck) {
    const healthCheckFn = options.healthCheckFn ?? defaultHealthCheck;
    try {
      await healthCheckFn();
    } catch (error: any) {
      const unavailable: ClaudeCodeAvailability = {
        available: false,
        reason: `Health check failed: ${error?.message || String(error)}`,
      };
      if (cliResult.path) unavailable.cliPath = cliResult.path;
      if (version) unavailable.version = version;
      return unavailable;
    }
  }

  const available: ClaudeCodeAvailability = {
    available: true,
  };
  if (cliResult.path) available.cliPath = cliResult.path;
  if (version) available.version = version;
  return available;
};
