import { createRequire } from "node:module";

export type ApiKeySource = "env" | "config" | "none";

export interface ClaudeCodeAvailability {
  available: boolean;
  version?: string;
  apiKeySource?: ApiKeySource;
  reason?: string;
}

export interface DetectClaudeCodeOptions {
  /** Override environment (defaults to process.env) */
  env?: Record<string, string | undefined>;
  /** Whether to run an optional health check (defaults to false) */
  healthCheck?: boolean;
  /** Custom health check (used in tests to avoid network calls) */
  healthCheckFn?: () => Promise<void>;
  /** Custom SDK resolver (used in tests to simulate presence/absence) */
  sdkResolver?: () => Promise<{ version?: string }>;
}

const defaultSdkResolver = async (): Promise<{ version?: string }> => {
  const require = createRequire(import.meta.url);
  const pkg = require("@anthropic-ai/claude-agent-sdk/package.json") as { version?: string };
  return { version: pkg.version };
};

const defaultHealthCheck = async (): Promise<void> => {
  const sdk = await import("@anthropic-ai/claude-agent-sdk");
  if (typeof (sdk as any).query !== "function") {
    throw new Error("query export not found");
  }
};

/**
 * Detect whether Claude Code/Agent SDK is available for use.
 * Checks SDK installation, API key presence, and optional health check hook.
 */
export const detectClaudeCode = async (
  options?: DetectClaudeCodeOptions
): Promise<ClaudeCodeAvailability> => {
  const env = options?.env ?? process.env;
  const resolveSdk = options?.sdkResolver ?? defaultSdkResolver;

  let version: string | undefined;
  try {
    const sdk = await resolveSdk();
    version = sdk.version;
  } catch {
    return {
      available: false,
      apiKeySource: "none",
      reason: "SDK not installed: bun add -E @anthropic-ai/claude-agent-sdk",
    };
  }

  const apiKey = env.ANTHROPIC_API_KEY || env.CLAUDE_API_KEY;
  const apiKeySource: ApiKeySource = apiKey ? "env" : "none";

  if (!apiKey) {
    return {
      available: false,
      version,
      apiKeySource,
      reason: "ANTHROPIC_API_KEY not set",
    };
  }

  if (options?.healthCheck) {
    const healthCheckFn = options.healthCheckFn ?? defaultHealthCheck;
    try {
      await healthCheckFn();
    } catch (error: any) {
      return {
        available: false,
        version,
        apiKeySource,
        reason: `Health check failed: ${error?.message || String(error)}`,
      };
    }
  }

  return {
    available: true,
    version,
    apiKeySource,
  };
};
