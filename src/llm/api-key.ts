import { homedir } from "os";
import { join } from "path";
import { existsSync, readFileSync, writeFileSync } from "fs";

const KEY_PATH = () => join(process.env.PI_CODING_AGENT_DIR || join(homedir(), ".pi", "agent"), "keys.json");

export type ProviderKey = {
  provider: string;
  apiKey: string;
};

const loadKeys = () => {
  const path = KEY_PATH();
  if (!existsSync(path)) return {};
  try {
    return JSON.parse(readFileSync(path, "utf8")) as Record<string, string>;
  } catch {
    return {};
  }
};

export const resolveApiKey = (provider: string, explicit?: string): string | null => {
  if (explicit) return explicit;
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const envVar =
    provider === "anthropic"
      ? env.ANTHROPIC_API_KEY
      : provider === "openai"
        ? env.OPENAI_API_KEY
        : provider === "google"
          ? env.GEMINI_API_KEY || env.GOOGLE_API_KEY
          : provider === "groq"
            ? env.GROQ_API_KEY
            : provider === "cerebras"
              ? env.CEREBRAS_API_KEY
              : provider === "xai"
                ? env.XAI_API_KEY
                : provider === "openrouter"
                  ? env.OPENROUTER_API_KEY
                  : null;
  if (envVar) return envVar;
  const stored = loadKeys();
  return stored[provider] ?? null;
};

export const cacheApiKey = (provider: string, apiKey: string): void => {
  const path = KEY_PATH();
  let data: Record<string, string> = {};
  if (existsSync(path)) {
    try {
      data = JSON.parse(readFileSync(path, "utf8"));
    } catch {
      data = {};
    }
  }
  data[provider] = apiKey;
  writeFileSync(path, JSON.stringify(data, null, 2), "utf8");
};
