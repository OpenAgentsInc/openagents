import { existsSync, readFileSync, writeFileSync } from "fs";
import { homedir } from "os";
import { join, resolve } from "path";

export interface OAuthToken {
  provider: "anthropic";
  accessToken: string;
  refreshToken?: string;
  expiresAt?: number; // epoch ms
}

const defaultPath = () => {
  const base = process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent");
  return resolve(base, "oauth.json");
};

export const loadOAuthToken = (path = defaultPath()): OAuthToken | null => {
  if (!existsSync(path)) return null;
  try {
    const raw = readFileSync(path, "utf8");
    const parsed = JSON.parse(raw) as OAuthToken;
    if (parsed.provider !== "anthropic" || !parsed.accessToken) return null;
    if (parsed.expiresAt && parsed.expiresAt < Date.now()) return null;
    return parsed;
  } catch {
    return null;
  }
};

export const saveOAuthToken = (token: OAuthToken, path = defaultPath()): void => {
  const dir = resolve(path, "..");
  if (!existsSync(dir)) {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const fs = require("fs") as typeof import("fs");
    fs.mkdirSync(dir, { recursive: true });
  }
  writeFileSync(path, JSON.stringify(token, null, 2), "utf8");
};

export const resolveAnthropicAuth = (explicit?: string): string | null => {
  if (explicit) return explicit;
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  if (env.ANTHROPIC_API_KEY) return env.ANTHROPIC_API_KEY;
  const stored = loadOAuthToken();
  return stored?.accessToken ?? null;
};
