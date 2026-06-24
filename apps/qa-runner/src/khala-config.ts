// Endpoint config for the Khala driver — env-driven, NO hardcoded secrets.
//
// Resolution order for the credential (first hit wins), per the build spec:
//   1. OPENAGENTS_API_KEY env var          -> real openagents/khala endpoint
//   2. an OpenAgents agent token discovered in ~/work/.secrets/*.env
//      (OPENAGENTS_AGENT_TOKEN=...)         -> real openagents/khala endpoint
//   3. PROBE_OPENAI_API_KEY env var OR ~/work/.secrets/probe-openai.env
//      (OpenAI-compatible)                  -> FALLBACK model, loop-proof only
//
// The fallback exists ONLY to prove the agent loop runs for real when no
// OpenAgents key is present; a fallback run is clearly labeled. The real
// openagents/khala endpoint is always preferred.
//
// The credential value is never printed; only its SOURCE label is logged.

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import type { ChatClient, ChatMessage } from "./khala-driver";

export interface KhalaEndpointConfig {
  readonly baseUrl: string;
  readonly apiKey: string;
  readonly model: string;
  /** Where the credential came from (label only, never the value). */
  readonly keySource: string;
  /** "khala" = real openagents/khala endpoint; "fallback" = OpenAI-compat proof. */
  readonly mode: "khala" | "fallback";
}

const DEFAULT_KHALA_BASE = "https://openagents.com/api/v1";
const DEFAULT_KHALA_MODEL = "openagents/khala";
const DEFAULT_OPENAI_BASE = "https://api.openai.com/v1";
const DEFAULT_FALLBACK_MODEL = "gpt-4o-mini";

/** Read `KEY=value` from a dotenv-style file (no value printing). Returns undefined. */
function readEnvValue(file: string, key: string): string | undefined {
  if (!existsSync(file)) return undefined;
  for (const line of readFileSync(file, "utf8").split("\n")) {
    const m = /^\s*([A-Z0-9_]+)\s*=\s*(.*)$/.exec(line);
    if (m && m[1] === key) {
      return m[2]!.trim().replace(/^["']|["']$/g, "");
    }
  }
  return undefined;
}

/** Find the first OpenAgents agent token in ~/work/.secrets/*.env, if any. */
function discoverAgentToken(secretsDir: string): { token: string; source: string } | undefined {
  if (!existsSync(secretsDir)) return undefined;
  let files: string[];
  try {
    files = readdirSync(secretsDir).filter((f) => f.endsWith(".env")).sort();
  } catch {
    return undefined;
  }
  for (const file of files) {
    const path = join(secretsDir, file);
    const token = readEnvValue(path, "OPENAGENTS_AGENT_TOKEN");
    if (token && token.length > 0) return { token, source: `~/work/.secrets/${file}#OPENAGENTS_AGENT_TOKEN` };
  }
  return undefined;
}

export interface ResolveKhalaConfigOptions {
  readonly env?: Readonly<Record<string, string | undefined>>;
  /** Override the secrets dir (tests point this at a temp dir). */
  readonly secretsDir?: string;
  /** Allow the OpenAI-compatible fallback (default true). */
  readonly allowFallback?: boolean;
}

/**
 * Resolve the Khala endpoint config from env + the secrets directory. Throws if
 * no usable credential is found (the demo must not pretend to run a model).
 */
export function resolveKhalaConfig(options: ResolveKhalaConfigOptions = {}): KhalaEndpointConfig {
  const env = options.env ?? process.env;
  const secretsDir = options.secretsDir ?? join(homedir(), "work", ".secrets");
  const allowFallback = options.allowFallback ?? true;

  const model = env.KHALA_MODEL ?? DEFAULT_KHALA_MODEL;
  const base = env.KHALA_BASE_URL ?? DEFAULT_KHALA_BASE;

  // 1. explicit OpenAgents API key
  if (env.OPENAGENTS_API_KEY && env.OPENAGENTS_API_KEY.length > 0) {
    return { baseUrl: base, apiKey: env.OPENAGENTS_API_KEY, model, keySource: "OPENAGENTS_API_KEY env", mode: "khala" };
  }

  // 2. discovered OpenAgents agent token in the secrets dir
  const discovered = discoverAgentToken(secretsDir);
  if (discovered) {
    return { baseUrl: base, apiKey: discovered.token, model, keySource: discovered.source, mode: "khala" };
  }

  // 3. OpenAI-compatible fallback (loop-proof only)
  if (allowFallback) {
    const fbBase = env.OPENAI_BASE_URL ?? DEFAULT_OPENAI_BASE;
    const fbModel = env.OPENAI_MODEL ?? DEFAULT_FALLBACK_MODEL;
    const fbKey =
      (env.PROBE_OPENAI_API_KEY && env.PROBE_OPENAI_API_KEY.length > 0
        ? env.PROBE_OPENAI_API_KEY
        : undefined) ?? readEnvValue(join(secretsDir, "probe-openai.env"), "PROBE_OPENAI_API_KEY");
    if (fbKey) {
      return {
        baseUrl: fbBase,
        apiKey: fbKey,
        model: fbModel,
        keySource: env.PROBE_OPENAI_API_KEY ? "PROBE_OPENAI_API_KEY env" : "~/work/.secrets/probe-openai.env",
        mode: "fallback",
      };
    }
  }

  throw new Error(
    "no Khala credential available: set OPENAGENTS_API_KEY (preferred, real openagents/khala) " +
      "or provide an OpenAgents agent token / PROBE_OPENAI_API_KEY fallback in ~/work/.secrets/.",
  );
}

/**
 * A `fetch`-based OpenAI-compatible chat client. No new dependency. A per-request
 * timeout aborts a stuck inference call. `max_tokens` defaults high because the
 * served gpt-oss-20b spends tokens on a reasoning channel before emitting
 * content (a tiny budget yields empty content / finish_reason=length).
 */
export function makeFetchChatClient(
  config: KhalaEndpointConfig,
  options: { readonly timeoutMs?: number; readonly maxTokens?: number } = {},
): ChatClient {
  const timeoutMs = options.timeoutMs ?? 60_000;
  const maxTokens = options.maxTokens ?? 512;
  const url = `${config.baseUrl.replace(/\/$/, "")}/chat/completions`;
  return {
    complete: async (messages: ReadonlyArray<ChatMessage>) => {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const response = await fetch(url, {
          method: "POST",
          headers: { "content-type": "application/json", authorization: `Bearer ${config.apiKey}` },
          body: JSON.stringify({ model: config.model, messages, temperature: 0, max_tokens: maxTokens }),
          signal: controller.signal,
        });
        if (!response.ok) {
          const body = await response.text().catch(() => "");
          throw new Error(`chat/completions HTTP ${response.status}: ${body.slice(0, 300)}`);
        }
        const json = (await response.json()) as { choices?: Array<{ message?: { content?: string } }> };
        return json.choices?.[0]?.message?.content ?? "";
      } finally {
        clearTimeout(timer);
      }
    },
  };
}
